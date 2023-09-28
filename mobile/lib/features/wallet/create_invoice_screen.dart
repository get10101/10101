import 'dart:math';

import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/wallet/domain/share_invoice.dart';
import 'package:get_10101/features/wallet/share_invoice_screen.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';
import 'package:get_10101/common/domain/channel.dart';
import 'package:get_10101/common/scrollable_safe_area.dart';

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

  /// The channel info if a channel already exists
  ///
  /// If no channel exists yet this field will be null.
  ChannelInfo? channelInfo;

  /// The max channel capacity as received by the LSP or if there is an existing channel
  Amount? lspMaxChannelCapacity;

  /// A reserve which needs to be allocated for paying transactions fees
  Amount? contractFeeReserve;

  /// Estimated fees for receiving
  ///
  /// These fees have to be added on top of the receive amount because they are collected after receiving the funds.
  Amount? feeEstimate;

  @override
  void dispose() {
    _amountController.dispose();
    super.dispose();
  }

  @override
  void initState() {
    final ChannelInfoService channelInfoService = context.read<ChannelInfoService>();
    initChannelInfo(channelInfoService);

    final bridge.Config config = context.read<bridge.Config>();
    amount = config.network == "regtest" ? Amount(100000) : null;

    super.initState();
  }

  Future<void> initChannelInfo(ChannelInfoService channelInfoService) async {
    channelInfo = await channelInfoService.getChannelInfo();
    lspMaxChannelCapacity = await channelInfoService.getMaxCapacity();
    contractFeeReserve = await channelInfoService.getTradeFeeReserve();

    // initial channel opening
    if (channelInfo == null) {
      feeEstimate = await channelInfoService.getChannelOpenFeeEstimate();
    }
  }

  @override
  Widget build(BuildContext context) {
    final ChannelInfoService channelInfoService = context.read<ChannelInfoService>();
    final WalletChangeNotifier walletChangeNotifier = context.watch<WalletChangeNotifier>();
    Amount balance = walletChangeNotifier.walletInfo.balances.lightning;

    Amount minTradeMargin = channelInfoService.getMinTradeMargin();
    Amount tradeFeeReserve = contractFeeReserve ?? Amount(0);
    Amount maxChannelCapacity = lspMaxChannelCapacity ?? Amount(0);
    Amount initialReserve = channelInfoService.getInitialReserve();

    int coordinatorLiquidityMultiplier = channelInfoService.getCoordinatorLiquidityMultiplier();

    // if we already have a channel we base the calculation on the channel capacity, otherwise we use the maximum channel capacity
    Amount channelCapacity = channelInfo?.channelCapacity ?? maxChannelCapacity;
    Amount maxAllowedOutboundCapacity =
        Amount((channelCapacity.sats / coordinatorLiquidityMultiplier).floor());

    // the minimum amount that has to be in the wallet to be able to trade
    Amount minAmountToBeAbleToTrade = Amount((channelInfo?.reserve.sats ?? initialReserve.sats) +
        tradeFeeReserve.sats +
        minTradeMargin.sats +
        // make sure that the amount received covers potential fees as well
        (feeEstimate?.sats ?? 0));

    // it can go below 0 if the user has an unbalanced channel
    Amount maxReceiveAmount = Amount(max(maxAllowedOutboundCapacity.sats - balance.sats, 0));

    // we have to at least receive enough to be able to trade with the minimum trade amount
    Amount minReceiveAmount = Amount(max(minAmountToBeAbleToTrade.sats - balance.sats, 1));

    return Scaffold(
      appBar: AppBar(title: const Text("Receive funds on Lightning")),
      body: Form(
        key: _formKey,
        child: GestureDetector(
          onTap: () {
            FocusScope.of(context).requestFocus(FocusNode());
          },
          behavior: HitTestBehavior.opaque,
          child: ScrollableSafeArea(
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
                        isLoading: lspMaxChannelCapacity == null,
                        infoText:
                            "While in beta, maximum channel capacity is limited to ${formatSats(maxChannelCapacity)}; channels above this capacity might get rejected."
                            "\nThe maximum is enforced initially to ensure users only trade with small stakes until the software has proven to be stable."
                            "\n\nYour current balance is ${formatSats(balance)}, so you can receive up to ${formatSats(maxReceiveAmount)}."
                            "\nIf you hold less than ${formatSats(minAmountToBeAbleToTrade)} or more than ${formatSats(maxAllowedOutboundCapacity)} in your wallet you might not be able to trade.",
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
                  ],
                ),
              ),
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

                                    walletChangeNotifier.service
                                        .createInvoice(amount!)
                                        .then((invoice) {
                                      if (invoice != null) {
                                        GoRouter.of(context).go(ShareInvoiceScreen.route,
                                            extra: ShareInvoice(
                                                rawInvoice: invoice,
                                                invoiceAmount: amount!,
                                                isLightning: true,
                                                channelOpenFee: feeEstimate));
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
