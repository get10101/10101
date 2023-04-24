import 'dart:math';

import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/application/channel_constraints_service.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/wallet/share_invoice_screen.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';
import 'package:get_10101/common/modal_bottom_sheet_info.dart';
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
  final ChannelConstraintsService channelConstraintsService = const ChannelConstraintsService();

  @override
  void dispose() {
    _amountController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    int channelCapacity = channelConstraintsService.getLightningChannelCapacity();
    int usableChannelCapacity = channelConstraintsService.getUsableChannelCapacity();
    int balance = context.watch<WalletChangeNotifier>().walletInfo.balances.lightning.sats;
    bool hasOpenPosition =
        context.watch<PositionChangeNotifier>().positions[ContractSymbol.btcusd] != null;
    // it can go below 0 if the user has an unbalanced channel
    int maxAmount = max(usableChannelCapacity - balance, 0);

    // if we already have a balance that is > 5666 then 1 is the minimum to receive
    int minAmount = max(
        channelConstraintsService.getChannelReserve() +
            channelConstraintsService.getFeeReserve() +
            channelConstraintsService.getMinTradeMargin() -
            balance,
        1);

    return Scaffold(
      appBar: AppBar(title: const Text("Receive funds")),
      body: Form(
        key: _formKey,
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
                        hint: "e.g. 5000 sats",
                        label: "Amount",
                        controller: _amountController,
                        onChanged: (value) {
                          if (value.isEmpty) {
                            return;
                          }

                          setState(() => amount = Amount.parse(value));
                        },
                        validator: (value) {
                          // FIXME: a temporary stop-gap for https://github.com/get10101/10101/issues/498
                          if (hasOpenPosition) {
                            return "Cannot receive funds whilst a position is open";
                          }

                          if (value == null) {
                            return "Enter receive amount";
                          }

                          try {
                            int amount = int.parse(value);

                            if (balance > usableChannelCapacity) {
                              return "Maximum beta balance exceeded";
                            }

                            if (amount < minAmount) {
                              return "Min amount to receive is $minAmount";
                            }

                            if (amount > maxAmount) {
                              return "Max amount to receive is $maxAmount";
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
                              "While in beta, channel capacity is limited to $channelCapacity sats; payments above this capacity might get rejected."
                              "\n\nYour current balance is $balance, so you can receive up to $maxAmount sats."
                              "\nIf you hold less than $minAmount or more than $usableChannelCapacity in your wallet you might not be able to trade."
                              "\n\nThe maximum is enforced initially to ensure users only trade with small stakes until the software has proven to be stable.",
                          buttonText: "Back to Receive..."),
                  ],
                ),
              ),
              Center(
                  child: Padding(
                padding: const EdgeInsets.only(bottom: 10.0, left: 32.0, right: 32.0),
                child: Text(
                  "During the beta we recommend a maximum wallet balance of 100000 sats."
                  "\nYour wallet balance is $balance sats so you should only receive up to $maxAmount sats.",
                  style: const TextStyle(color: Colors.grey),
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
    );
  }
}
