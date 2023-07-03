import 'dart:math';

import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/wallet/share_invoice_screen.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';
import 'package:get_10101/common/modal_bottom_sheet_info.dart';
import '../../common/domain/channel.dart';
import 'application/wallet_service.dart';

class CreateInvoiceScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "create_invoice";

  const CreateInvoiceScreen({super.key});

  @override
  State<CreateInvoiceScreen> createState() => _CreateInvoiceScreenState();
}

class _CreateInvoiceScreenState extends State<CreateInvoiceScreen> {
  Amount? amount;
  final TextEditingController _amountController = TextEditingController();

  final _formKey = GlobalKey<FormState>();
  bool showValidationHint = false;

  final WalletService walletService = const WalletService();

  final ChannelInfoService channelInfoService = const ChannelInfoService();
  ChannelInfo? channelInfo;

  @override
  void dispose() {
    _amountController.dispose();
    super.dispose();
  }

  @override
  void initState() {
    initChannelInfo();
    super.initState();
  }

  Future<void> initChannelInfo() async {
    channelInfo = await channelInfoService.getChannelInfo();
  }

  @override
  Widget build(BuildContext context) {
    Amount balance = context.watch<WalletChangeNotifier>().walletInfo.balances.lightning;

    Amount minTradeMargin = channelInfoService.getMinTradeMargin();
    Amount tradeFeeReserve = channelInfoService.getTradeFeeReserve();
    Amount maxChannelCapacity = channelInfoService.getMaxCapacity();
    Amount initialReserve = channelInfoService.getInitialReserve();

    // if we already have a channel we base the calculation on the channel capacity, otherwise we use the maximum channel capacity
    Amount channelCapacity = channelInfo?.channelCapacity ?? maxChannelCapacity;
    Amount maxAllowedOutboundCapacity = Amount((channelCapacity.sats / 2).floor());

    // it can go below 0 if the user has an unbalanced channel
    Amount maxReceiveAmount = Amount(max(maxAllowedOutboundCapacity.sats - balance.sats, 0));

    // we have to at least receive enough to be able to trade with the minimum trade amount
    Amount minReceiveAmount = Amount(max(
        (channelInfo?.reserve.sats ?? initialReserve.sats) +
            tradeFeeReserve.sats +
            minTradeMargin.sats -
            balance.sats,
        1));

    return Scaffold(
      appBar: AppBar(title: const Text("Receive funds")),
      body: Form(
        key: _formKey,
        child: GestureDetector(
          onTap: () {
            FocusScope.of(context).requestFocus(FocusNode());
          },
          behavior: HitTestBehavior.opaque,
          child: SafeArea(
            child: Container(
              constraints: const BoxConstraints.expand(),
              child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                const Center(
                    child: Padding(
                        padding: EdgeInsets.only(top: 25.0),
                        child: Text(
                          "What is the amount?",
                          style: TextStyle(fontSize: 18.0, fontWeight: FontWeight.bold),
                        ))),
                Padding(
                  padding: const EdgeInsets.all(32.0),
                  child: Row(
                    children: [
                      Expanded(
                        child: AmountInputField(
                          value: amount != null ? amount! : Amount(0),
                          hint: "e.g. ${formatSats(Amount(5000))}",
                          label: "Amount",
                          controller: _amountController,
                          onChanged: (value) {
                            if (value.isEmpty) {
                              return;
                            }

                            setState(() => amount = Amount.parse(value));
                          },
                          validator: (value) {
                            if (value == null) {
                              return "Enter receive amount";
                            }

                            try {
                              int amount = int.parse(value);

                              if (balance.sats > maxAllowedOutboundCapacity.sats) {
                                return "Maximum channel balance exceeded";
                              }

                              if (amount < minReceiveAmount.sats) {
                                return "Min amount to receive is ${formatSats(minReceiveAmount)}";
                              }

                              if (amount > maxReceiveAmount.sats) {
                                return "Max amount to receive is ${formatSats(maxReceiveAmount)}";
                              }
                            } on Exception {
                              return "Enter a number";
                            }

                            return null;
                          },
                        ),
                      ),
                      if (showValidationHint)
                        ModalBottomSheetInfo(
                            infoText:
                                "While in beta, maximum channel capacity is limited to ${formatSats(maxChannelCapacity)}; channels above this capacity might get rejected."
                                "\nThe maximum is enforced initially to ensure users only trade with small stakes until the software has proven to be stable."
                                "\n\nYour current balance is ${formatSats(balance)}, so you can receive up to ${formatSats(maxReceiveAmount)}."
                                "\nIf you hold less than ${formatSats(minReceiveAmount)} or more than ${formatSats(maxAllowedOutboundCapacity)} in your wallet you might not be able to trade.",
                            buttonText: "Back to Receive..."),
                    ],
                  ),
                ),
                Center(
                    child: Padding(
                  padding: const EdgeInsets.only(bottom: 10.0, left: 32.0, right: 32.0),
                  child: Text(
                    "Your wallet balance is ${formatSats(balance)} so you should only receive up to ${formatSats(maxReceiveAmount)}.",
                    style: const TextStyle(color: Colors.black),
                  ),
                )),
                Expanded(
                  child: Padding(
                    padding: const EdgeInsets.all(32.0),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      mainAxisAlignment: MainAxisAlignment.end,
                      children: [
                        ElevatedButton(
                            onPressed: (amount == null || amount == Amount(0))
                                ? null
                                : () {
                                    if (_formKey.currentState!.validate()) {
                                      showValidationHint = false;
                                      walletService.createInvoice(amount!).then((invoice) {
                                        if (invoice != null) {
                                          GoRouter.of(context)
                                              .go(ShareInvoiceScreen.route, extra: invoice);
                                        }
                                      });
                                    } else {
                                      setState(() {
                                        showValidationHint = true;
                                      });
                                    }
                                  },
                            child: const Text("Next")),
                      ],
                    ),
                  ),
                )
              ]),
            ),
          ),
        ),
      ),
    );
  }
}
