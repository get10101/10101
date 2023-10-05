import 'dart:async';
import 'dart:math';

import 'package:f_logs/f_logs.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/domain/channel.dart';
import 'package:get_10101/common/domain/liquidity_option.dart';
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
import 'package:get_10101/features/trade/trade_dialog.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
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
  late final ChannelInfoService channelInfoService;

  TextEditingController marginController = TextEditingController();
  TextEditingController quantityController = TextEditingController();
  TextEditingController priceController = TextEditingController();

  final _formKey = GlobalKey<FormState>();

  bool showCapacityInfo = false;

  @override
  void initState() {
    provider = context.read<TradeValuesChangeNotifier>();
    channelInfoService = provider.channelInfoService;
    super.initState();
  }

  Future<(ChannelInfo?, Amount, Amount, double)> _getChannelInfo(
      ChannelInfoService channelInfoService) async {
    var channelInfo = await channelInfoService.getChannelInfo();

    // fetching also inactive liquidity options as the user might use a liquidity option that isn't active anymore.
    final options = await channelInfoService.getLiquidityOptions(false);

    /// The max channel capacity of the existing channel. 0 if no channel exists.
    var lspMaxChannelCapacity = await channelInfoService.getMaxCapacity();
    var tradeReserve = await channelInfoService.getTradeFeeReserve();

    var completer = Completer<(ChannelInfo?, Amount, Amount, double)>();

    if (channelInfo?.liquidityOptionId != null) {
      final liquidityOption = options.singleWhere(
          (LiquidityOption option) => option.liquidityOptionId == channelInfo?.liquidityOptionId);
      completer.complete(
          (channelInfo, lspMaxChannelCapacity, tradeReserve, liquidityOption.coordinatorLeverage));
    } else {
      // channels created before 1.3.1 do not have a liquidity option, hence we use the default value.
      completer.complete((channelInfo, lspMaxChannelCapacity, tradeReserve, 1.0));
    }

    return completer.future;
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

    Direction direction = widget.direction;
    String label = direction == Direction.long ? "Buy" : "Sell";
    Color color = direction == Direction.long ? tradeTheme.buy : tradeTheme.sell;

    return Form(
      key: _formKey,
      child: Column(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        crossAxisAlignment: CrossAxisAlignment.center,
        mainAxisSize: MainAxisSize.min,
        children: [
          FutureBuilder<(ChannelInfo?, Amount, Amount, double)>(
            future:
                _getChannelInfo(channelInfoService), // a previously-obtained Future<String> or null
            builder: (BuildContext context,
                AsyncSnapshot<(ChannelInfo?, Amount, Amount, double)> snapshot) {
              List<Widget> children;

              if (snapshot.hasData) {
                var (channelInfo, lspMaxChannelCapacity, tradeFeeReserve, coordinatorLeverage) =
                    snapshot.data!;
                Amount minTradeMargin = channelInfoService.getMinTradeMargin();

                Amount channelCapacity = lspMaxChannelCapacity;

                Amount initialReserve = channelInfoService.getInitialReserve();

                Amount channelReserve = channelInfo?.reserve ?? initialReserve;
                int totalReserve = channelReserve.sats + tradeFeeReserve.sats;

                int usableBalance = max(walletInfo.balances.lightning.sats - totalReserve, 0);
                // the assumed balance of the counterparty based on the channel and our balance
                // this is needed to make sure that the counterparty can fulfil the trade
                int counterpartyUsableBalance = max(
                    channelCapacity.sats - (walletInfo.balances.lightning.sats + totalReserve), 0);
                int maxMargin = usableBalance;

                // the trading capacity does not take into account if the channel is balanced or not
                int tradingCapacity = channelCapacity.sats -
                    totalReserve -
                    (provider.counterpartyMargin(widget.direction, coordinatorLeverage) ?? 0);

                children = <Widget>[
                  buildChildren(
                      usableBalance,
                      totalReserve,
                      channelReserve,
                      tradeFeeReserve,
                      direction,
                      tradingCapacity,
                      counterpartyUsableBalance,
                      maxMargin,
                      minTradeMargin,
                      channelCapacity,
                      coordinatorLeverage,
                      context,
                      channelInfoService),
                ];
              } else if (snapshot.hasError) {
                children = <Widget>[
                  const Icon(
                    Icons.error_outline,
                    color: Colors.red,
                    size: 60,
                  ),
                  Padding(
                    padding: const EdgeInsets.only(top: 16),
                    child:
                        Text('Error: Could not load confirmation screen due to ${snapshot.error}'),
                  ),
                ];
              } else {
                children = const <Widget>[
                  SizedBox(
                    width: 60,
                    height: 60,
                    child: CircularProgressIndicator(),
                  ),
                  Padding(
                    padding: EdgeInsets.only(top: 16),
                    child: Text('Loading confirmation screen...'),
                  ),
                ];
              }
              return Center(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: children,
                ),
              );
            },
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

                            // Return to the trade screen before submitting the pending order so that the dialog is displayed under the correct context
                            GoRouter.of(context).pop();
                            GoRouter.of(context).pop();

                            // Show immediately the pending dialog, when submitting a market order.
                            // TODO(holzeis): We should only show the dialog once we've received a match.
                            showDialog(
                                context: context,
                                useRootNavigator: true,
                                barrierDismissible: false, // Prevent user from leaving
                                builder: (BuildContext context) {
                                  return const TradeDialog();
                                });
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

  Wrap buildChildren(
      int usableBalance,
      int totalReserve,
      Amount channelReserve,
      Amount tradeFeeReserve,
      Direction direction,
      int tradingCapacity,
      int counterpartyUsableBalance,
      int maxMargin,
      Amount minTradeMargin,
      Amount channelCapacity,
      double coordinatorLeverage,
      BuildContext context,
      ChannelInfoService channelInfoService) {
    return Wrap(
      runSpacing: 12,
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
                closeButtonText: "Back to order...",
                infoButtonPadding: const EdgeInsets.symmetric(horizontal: 8.0),
                child: Text(
                    "Your usable balance of $usableBalance sats takes a fixed reserve of $totalReserve sats into account. "
                    "\n${channelReserve.sats} is the minimum amount that has to stay in the Lightning channel. "
                    "\n${tradeFeeReserve.sats} is reserved for fees per trade that is needed for publishing on-chain transactions in a worst case scenario. This is needed for the self-custodial setup"
                    "\n\nWe are working on optimizing the reserve and it might be subject to change after the beta."),
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
                        context.read<TradeValuesChangeNotifier>().updateQuantity(direction, 0);
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
                selector: (_, provider) => provider.fromDirection(direction).margin ?? Amount(0),
                builder: (context, margin, child) {
                  return AmountInputField(
                    value: margin,
                    hint: "e.g. ${Amount(100000)}",
                    label: "Margin (sats)",
                    controller: marginController,
                    isLoading: false,
                    onChanged: (value) {
                      if (value.isEmpty) {
                        return;
                      }

                      try {
                        Amount margin = Amount.parseAmount(value);
                        context.read<TradeValuesChangeNotifier>().updateMargin(direction, margin);
                      } catch (error) {
                        FLog.error(text: "Error: $error");
                        context
                            .read<TradeValuesChangeNotifier>()
                            .updateMargin(direction, Amount.zero());
                      }
                    },
                    validator: (value) {
                      if (value == null) {
                        return "Enter margin";
                      }

                      Amount margin = Amount.parseAmount(value);

                      int? optCounterPartyMargin =
                          provider.counterpartyMargin(direction, coordinatorLeverage);
                      if (optCounterPartyMargin == null) {
                        return "Counterparty margin not available";
                      }
                      int counterpartyMargin = optCounterPartyMargin;

                      // This condition has to stay as the first thing to check, so we reset showing the info
                      if (margin.sats > tradingCapacity ||
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
                      if (usableBalance < margin.sats + fee.sats) {
                        return "Insufficient balance";
                      }

                      if (margin.sats > maxMargin) {
                        return "Max margin is $maxMargin";
                      }
                      if (margin.sats < minTradeMargin.sats) {
                        return "Min margin is ${minTradeMargin.sats}";
                      }

                      return null;
                    },
                  );
                },
              ),
            ),
            if (showCapacityInfo)
              ModalBottomSheetInfo(
                  closeButtonText: "Back to order...",
                  child: Text(
                      "Your channel capacity is limited to $channelCapacity sats. During the beta channel resize is not available yet"
                      "In order to trade with higher margin you have to reduce your balance"
                      "\n\nYour current usable balance is $usableBalance."))
          ],
        ),
        LeverageSlider(
            initialValue: context
                .read<TradeValuesChangeNotifier>()
                .fromDirection(direction)
                .leverage
                .leverage,
            onLeverageChanged: (value) {
              context.read<TradeValuesChangeNotifier>().updateLeverage(direction, Leverage(value));
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
            const SizedBox(width: 16),
            const Flexible(child: Text("Estimated fee:")),
            const SizedBox(width: 5),
            Selector<TradeValuesChangeNotifier, Amount>(
                selector: (_, provider) => provider.orderMatchingFee(direction) ?? Amount.zero(),
                builder: (context, fee, child) {
                  return Flexible(child: AmountText(amount: fee));
                }),
          ],
        )
      ],
    );
  }
}
