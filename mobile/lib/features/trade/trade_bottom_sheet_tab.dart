import 'dart:math';

import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/double_text_input_form_field.dart';
import 'package:get_10101/common/fiat_text.dart';
import 'package:get_10101/common/modal_bottom_sheet_info.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/features/trade/domain/trade_values.dart';
import 'package:get_10101/features/trade/leverage_slider.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/trade/trade_bottom_sheet_confirmation.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:get_10101/features/wallet/domain/wallet_info.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

class TradeBottomSheetTab extends StatefulWidget {
  final Direction direction;
  final Key buttonKey;

  const TradeBottomSheetTab({required this.direction, super.key, required this.buttonKey});

  @override
  State<TradeBottomSheetTab> createState() => _TradeBottomSheetTabState();
}

class _TradeBottomSheetTabState extends State<TradeBottomSheetTab> {
  late final TradeValuesChangeNotifier provider;

  TextEditingController marginController = TextEditingController();
  TextEditingController quantityController = TextEditingController();
  TextEditingController priceController = TextEditingController();

  final _formKey = GlobalKey<FormState>();

  bool showCapacityInfo = false;

  @override
  void initState() {
    provider = context.read<TradeValuesChangeNotifier>();
    super.initState();
  }

  @override
  void dispose() {
    marginController.dispose();
    quantityController.dispose();
    priceController.dispose();

    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    TradeTheme tradeTheme = Theme.of(context).extension<TradeTheme>()!;

    WalletInfo walletInfo = context.watch<WalletChangeNotifier>().walletInfo;

    String label = widget.direction == Direction.long ? "Buy" : "Sell";
    Color color = widget.direction == Direction.long ? tradeTheme.buy : tradeTheme.sell;

    int minMargin = provider.minMargin;
    int usableBalance = max(walletInfo.balances.lightning.sats - provider.reserve, 0);
    // the assumed balance of the counterparty based on the channel and our balance
    // this is needed to make sure that the counterparty can fulfil the trade
    int counterpartyUsableBalance =
        max(provider.capacity - (walletInfo.balances.lightning.sats + provider.reserve), 0);
    int maxMargin = usableBalance;

    return Form(
      key: _formKey,
      child: Column(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        crossAxisAlignment: CrossAxisAlignment.center,
        mainAxisSize: MainAxisSize.min,
        children: [
          Wrap(
            runSpacing: 15,
            children: [
              Padding(
                padding: const EdgeInsets.only(bottom: 10),
                child: Row(
                  children: [
                    const Flexible(child: Text("Usable Balance:")),
                    const SizedBox(width: 5),
                    Flexible(child: AmountText(amount: Amount(usableBalance))),
                    const SizedBox(
                      width: 5,
                    ),
                    ModalBottomSheetInfo(
                      infoText:
                          "Your usable balance of $usableBalance sats takes a fixed reserve of ${provider.reserve} sats into account. "
                          "\n${provider.channelReserve} is the minimum amount that has to stay in the Lightning channel. "
                          "\n${provider.feeReserve} is reserved for fees per trade that is needed for publishing on-chain transactions in a worst case scenario. This is needed for the self-custodial setup"
                          "\n\nWe are working on optimizing the reserve and it might be subject to change after the beta.",
                      buttonText: "Back to order...",
                      padding: const EdgeInsets.symmetric(horizontal: 8.0),
                    )
                  ],
                ),
              ),
              Selector<TradeValuesChangeNotifier, double>(
                  selector: (_, provider) => provider.fromDirection(widget.direction).price,
                  builder: (context, price, child) {
                    return DoubleTextInputFormField(
                      value: price,
                      controller: priceController,
                      enabled: false,
                      label: "Market Price",
                    );
                  }),
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Flexible(
                    child: Selector<TradeValuesChangeNotifier, double>(
                      selector: (_, provider) => provider.fromDirection(widget.direction).quantity,
                      builder: (context, quantity, child) {
                        return DoubleTextInputFormField(
                          value: quantity,
                          hint: "e.g. 100 USD",
                          label: "Quantity (USD)",
                          controller: quantityController,
                          onChanged: (value) {
                            if (value.isEmpty) {
                              return;
                            }

                            try {
                              double quantity = double.parse(value);
                              context
                                  .read<TradeValuesChangeNotifier>()
                                  .updateQuantity(widget.direction, quantity);
                            } on Exception {
                              context
                                  .read<TradeValuesChangeNotifier>()
                                  .updateQuantity(widget.direction, 0);
                            }
                          },
                        );
                      },
                    ),
                  ),
                  const SizedBox(
                    width: 10,
                  ),
                  Flexible(
                    child: Selector<TradeValuesChangeNotifier, Amount>(
                      selector: (_, provider) => provider.fromDirection(widget.direction).margin,
                      builder: (context, margin, child) {
                        return AmountInputField(
                          value: margin,
                          hint: "e.g. 2000 sats",
                          label: "Margin (sats)",
                          controller: marginController,
                          onChanged: (value) {
                            if (value.isEmpty) {
                              return;
                            }

                            try {
                              Amount margin = Amount.parse(value);
                              context
                                  .read<TradeValuesChangeNotifier>()
                                  .updateMargin(widget.direction, margin);
                            } on Exception {
                              context
                                  .read<TradeValuesChangeNotifier>()
                                  .updateMargin(widget.direction, Amount.zero());
                            }
                          },
                          validator: (value) {
                            if (value == null) {
                              return "Enter margin";
                            }

                            try {
                              int margin = int.parse(value);

                              // the trading capacity does not take into account if the channel is balanced or not
                              int tradingCapacity =
                                  provider.availableTradingCapacity(widget.direction);
                              int counterpartyMargin =
                                  provider.counterpartyMargin(widget.direction);

                              // This condition has to stay as the first thing to check, so we reset showing the info
                              if (margin > tradingCapacity ||
                                  counterpartyMargin > counterpartyUsableBalance) {
                                setState(() {
                                  showCapacityInfo = true;
                                });

                                return "Insufficient capacity";
                              } else if (showCapacityInfo) {
                                setState(() {
                                  showCapacityInfo = false;
                                });
                              }

                              if (usableBalance < margin) {
                                return "Insufficient balance";
                              }

                              if (margin > maxMargin) {
                                return "Max margin is $maxMargin";
                              }
                              if (margin < minMargin) {
                                return "Min margin is $minMargin";
                              }
                            } on Exception {
                              return "Enter a number";
                            }

                            return null;
                          },
                        );
                      },
                    ),
                  ),
                  if (showCapacityInfo)
                    ModalBottomSheetInfo(
                        infoText:
                            "While in beta channel capacity is limited to ${provider.capacity} sats. "
                            "In order to trade with higher margin you have to reduce your balance"
                            "\n\nYour current usable balance is $usableBalance."
                            "Please send ${usableBalance - (provider.capacity / 2)} out of your wallet to free up capacity.",
                        buttonText: "Back to order...")
                ],
              ),
              LeverageSlider(
                  initialValue: context
                      .read<TradeValuesChangeNotifier>()
                      .fromDirection(widget.direction)
                      .leverage
                      .leverage,
                  onLeverageChanged: (value) {
                    context
                        .read<TradeValuesChangeNotifier>()
                        .updateLeverage(widget.direction, Leverage(value));
                  }),
              Row(
                children: [
                  const Flexible(child: Text("Liquidation Price:")),
                  const SizedBox(width: 5),
                  Selector<TradeValuesChangeNotifier, double>(
                      selector: (_, provider) =>
                          provider.fromDirection(widget.direction).liquidationPrice,
                      builder: (context, liquidationPrice, child) {
                        return Flexible(child: FiatText(amount: liquidationPrice));
                      }),
                ],
              )
            ],
          ),
          Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              ElevatedButton(
                  key: widget.buttonKey,
                  onPressed: () {
                    if (_formKey.currentState!.validate()) {
                      TradeValues tradeValues =
                          context.read<TradeValuesChangeNotifier>().fromDirection(widget.direction);
                      final submitOrderChangeNotifier = context.read<SubmitOrderChangeNotifier>();
                      tradeBottomSheetConfirmation(
                          context: context,
                          direction: widget.direction,
                          onConfirmation: () {
                            submitOrderChangeNotifier.submitPendingOrder(tradeValues, false);

                            // TODO: Explore if it would be easier / better handle the popups as routes
                            // Pop twice to navigate back to the trade screen.
                            GoRouter.of(context).pop();
                            GoRouter.of(context).pop();
                          });
                    }
                  },
                  style: ElevatedButton.styleFrom(
                      backgroundColor: color, minimumSize: const Size.fromHeight(50)),
                  child: Text(label)),
            ],
          )
        ],
      ),
    );
  }
}
