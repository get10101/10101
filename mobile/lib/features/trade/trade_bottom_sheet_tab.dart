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
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/domain/channel.dart';

class TradeBottomSheetTab extends StatefulWidget {
  final Direction direction;
  final Key buttonKey;

  const TradeBottomSheetTab({required this.direction, super.key, required this.buttonKey});

  @override
  State<TradeBottomSheetTab> createState() => _TradeBottomSheetTabState();
}

class _TradeBottomSheetTabState extends State<TradeBottomSheetTab> {
  late final TradeValuesChangeNotifier provider;
  late final ChannelInfoService channelInfoService;

  TextEditingController marginController = TextEditingController();
  TextEditingController quantityController = TextEditingController();
  TextEditingController priceController = TextEditingController();

  final _formKey = GlobalKey<FormState>();

  bool showCapacityInfo = false;

  ChannelInfo? channelInfo;

  @override
  void initState() {
    provider = context.read<TradeValuesChangeNotifier>();
    channelInfoService = provider.channelInfoService;
    initChannelInfo(channelInfoService);
    super.initState();
  }

  Future<void> initChannelInfo(ChannelInfoService channelInfoService) async {
    channelInfo = await channelInfoService.getChannelInfo();
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

    Amount minTradeMargin = channelInfoService.getMinTradeMargin();
    Amount tradeFeeReserve = channelInfoService.getTradeFeeReserve();
    Amount maxChannelCapacity = channelInfoService.getMaxCapacity();
    Amount initialReserve = channelInfoService.getInitialReserve();

    Direction direction = widget.direction;

    String label = direction == Direction.long ? "Buy" : "Sell";
    Color color = direction == Direction.long ? tradeTheme.buy : tradeTheme.sell;

    Amount channelReserve = channelInfo?.reserve ?? initialReserve;
    int totalReserve = channelReserve.sats + tradeFeeReserve.sats;

    int usableBalance = max(walletInfo.balances.lightning.sats - totalReserve, 0);
    Amount channelCapacity = channelInfo?.channelCapacity ?? maxChannelCapacity;
    // the assumed balance of the counterparty based on the channel and our balance
    // this is needed to make sure that the counterparty can fulfil the trade
    int counterpartyUsableBalance =
        max(channelCapacity.sats - (walletInfo.balances.lightning.sats + totalReserve), 0);
    int maxMargin = usableBalance;

    // the trading capacity does not take into account if the channel is balanced or not
    int tradingCapacity =
        channelCapacity.sats - totalReserve - (provider.counterpartyMargin(widget.direction) ?? 0);

    int coordinatorLiquidityMultiplier = channelInfoService.getCoordinatorLiquidityMultiplier();

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
                          "Your usable balance of $usableBalance sats takes a fixed reserve of $totalReserve sats into account. "
                          "\n${channelReserve.sats} is the minimum amount that has to stay in the Lightning channel. "
                          "\n${tradeFeeReserve.sats} is reserved for fees per trade that is needed for publishing on-chain transactions in a worst case scenario. This is needed for the self-custodial setup"
                          "\n\nWe are working on optimizing the reserve and it might be subject to change after the beta.",
                      buttonText: "Back to order...",
                      padding: const EdgeInsets.symmetric(horizontal: 8.0),
                    )
                  ],
                ),
              ),
              Selector<TradeValuesChangeNotifier, double>(
                  selector: (_, provider) => provider.fromDirection(direction).price ?? 0,
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
                      selector: (_, provider) => provider.fromDirection(direction).quantity ?? 0.0,
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
                                  .updateQuantity(direction, quantity);
                            } on Exception {
                              context
                                  .read<TradeValuesChangeNotifier>()
                                  .updateQuantity(direction, 0);
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
                      selector: (_, provider) =>
                          provider.fromDirection(direction).margin ?? Amount(0),
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
                                  .updateMargin(direction, margin);
                            } on Exception {
                              context
                                  .read<TradeValuesChangeNotifier>()
                                  .updateMargin(direction, Amount.zero());
                            }
                          },
                          validator: (value) {
                            if (value == null) {
                              return "Enter margin";
                            }

                            try {
                              int margin = int.parse(value);

                              int? optCounterPartyMargin = provider.counterpartyMargin(direction);
                              if (optCounterPartyMargin == null) {
                                return "Counterparty margin not available";
                              }
                              int counterpartyMargin = optCounterPartyMargin;

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

                              Amount fee = provider.orderMatchingFee(direction) ?? Amount.zero();
                              if (usableBalance < margin + fee.sats) {
                                return "Insufficient balance";
                              }

                              if (margin > maxMargin) {
                                return "Max margin is $maxMargin";
                              }
                              if (margin < minTradeMargin.sats) {
                                return "Min margin is ${minTradeMargin.sats}";
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
                            "Your channel capacity is limited to $channelCapacity sats. During the beta channel resize is not available yet"
                            "In order to trade with higher margin you have to reduce your balance"
                            "\n\nYour current usable balance is $usableBalance."
                            "Please send ${usableBalance - (channelCapacity.sats / coordinatorLiquidityMultiplier)} sats out of your wallet to free up capacity.",
                        buttonText: "Back to order...")
                ],
              ),
              LeverageSlider(
                  initialValue: context
                      .read<TradeValuesChangeNotifier>()
                      .fromDirection(direction)
                      .leverage
                      .leverage,
                  onLeverageChanged: (value) {
                    context
                        .read<TradeValuesChangeNotifier>()
                        .updateLeverage(direction, Leverage(value));
                  }),
              Row(
                children: [
                  const Flexible(child: Text("Liquidation Price:")),
                  const SizedBox(width: 5),
                  Selector<TradeValuesChangeNotifier, double>(
                      selector: (_, provider) =>
                          provider.fromDirection(direction).liquidationPrice ?? 0.0,
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
                          context.read<TradeValuesChangeNotifier>().fromDirection(direction);
                      final submitOrderChangeNotifier = context.read<SubmitOrderChangeNotifier>();
                      tradeBottomSheetConfirmation(
                          context: context,
                          direction: direction,
                          onConfirmation: () {
                            submitOrderChangeNotifier.submitPendingOrder(
                                tradeValues, PositionAction.open);

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
