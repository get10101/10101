import 'dart:async';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/amount_text_field.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/application/lsp_change_notifier.dart';
import 'package:get_10101/common/domain/channel.dart';
import 'package:get_10101/common/domain/liquidity_option.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/modal_bottom_sheet_info.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/features/trade/domain/trade_values.dart';
import 'package:get_10101/features/trade/leverage_slider.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/trade/trade_bottom_sheet_confirmation.dart';
import 'package:get_10101/features/trade/trade_dialog.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:get_10101/features/wallet/domain/wallet_info.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

const contractSymbol = ContractSymbol.btcusd;

class TradeBottomSheetTab extends StatefulWidget {
  final Direction direction;
  final Key buttonKey;

  const TradeBottomSheetTab({required this.direction, super.key, required this.buttonKey});

  @override
  State<TradeBottomSheetTab> createState() => _TradeBottomSheetTabState();
}

class _TradeBottomSheetTabState extends State<TradeBottomSheetTab> {
  late final TradeValuesChangeNotifier provider;
  late final LspChangeNotifier lspChangeNotifier;
  late final PositionChangeNotifier positionChangeNotifier;

  TextEditingController marginController = TextEditingController();
  TextEditingController quantityController = TextEditingController();
  TextEditingController priceController = TextEditingController();

  final _formKey = GlobalKey<FormState>();

  bool showCapacityInfo = false;

  @override
  void initState() {
    provider = context.read<TradeValuesChangeNotifier>();
    lspChangeNotifier = context.read<LspChangeNotifier>();
    positionChangeNotifier = context.read<PositionChangeNotifier>();
    super.initState();
  }

  Future<(ChannelInfo?, Amount, Amount, double)> _getChannelInfo(
      LspChangeNotifier lspChangeNotifier) async {
    final channelInfoService = lspChangeNotifier.channelInfoService;
    var channelInfo = await channelInfoService.getChannelInfo();

    // fetching also inactive liquidity options as the user might use a liquidity option that isn't active anymore.
    final options = lspChangeNotifier.getLiquidityOptions(false);

    /// The max channel capacity of the existing channel. 0 if no channel exists.
    var lspMaxChannelCapacity = await channelInfoService.getMaxCapacity();
    var tradeReserve = await lspChangeNotifier.getTradeFeeReserve();

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
                _getChannelInfo(lspChangeNotifier), // a previously-obtained Future<String> or null
            builder: (BuildContext context,
                AsyncSnapshot<(ChannelInfo?, Amount, Amount, double)> snapshot) {
              List<Widget> children;

              final channelInfoService = lspChangeNotifier.channelInfoService;

              if (snapshot.hasData) {
                var (channelInfo, lspMaxChannelCapacity, tradeFeeReserve, coordinatorLeverage) =
                    snapshot.data!;
                Amount minTradeMargin = channelInfoService.getMinTradeMargin();

                Amount channelCapacity = lspMaxChannelCapacity;

                Amount initialReserve = channelInfoService.getInitialReserve();

                Amount channelReserve = channelInfo?.reserve ?? initialReserve;
                int totalReserve = channelReserve.sats + tradeFeeReserve.sats;

                // If there is an open position then we can use all those funds to resize the
                // position in the _opposite_ direction.
                int usableMarginInPosition =
                    positionChangeNotifier.marginUsableForTrade(direction).sats;

                int usableBalance = max(
                    walletInfo.balances.lightning.sats + usableMarginInPosition - totalReserve, 0);

                // The assumed balance of the counterparty based on the channel and our balance. This
                // is needed to make sure that the counterparty can fulfil the trade.
                int counterpartyUsableBalance = max(
                    channelCapacity.sats - (walletInfo.balances.lightning.sats + totalReserve), 0);
                int maxMargin = usableBalance;

                children = <Widget>[
                  buildChildren(
                      usableBalance,
                      totalReserve,
                      channelReserve,
                      tradeFeeReserve,
                      direction,
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
      int counterpartyUsableBalance,
      int maxMargin,
      Amount minTradeMargin,
      Amount channelCapacity,
      double coordinatorLeverage,
      BuildContext context,
      ChannelInfoService channelInfoService) {
    final tradeValues = context.read<TradeValuesChangeNotifier>().fromDirection(direction);

    bool hasPosition = positionChangeNotifier.positions.containsKey(contractSymbol);

    double? positionLeverage;
    if (hasPosition) {
      final position = context.read<PositionChangeNotifier>().positions[contractSymbol];
      positionLeverage = position!.leverage.leverage;
    }

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
                closeButtonText: "Back to order",
                infoButtonPadding: const EdgeInsets.symmetric(horizontal: 8.0),
                child: Text(
                    "Your usable balance of ${formatSats(Amount(usableBalance))} sats takes a fixed reserve of ${formatSats(Amount(totalReserve))} sats into account. "
                    "\n${formatSats(channelReserve)} is the minimum amount that has to stay in the Lightning channel. "
                    "\n${formatSats(tradeFeeReserve)} is reserved for fees per trade that is needed for publishing on-chain transactions in a worst case scenario. This is needed for the self-custodial setup"
                    "\n\nWe are working on optimizing the reserve and it might be subject to change after the beta."),
              )
            ],
          ),
        ),
        Selector<TradeValuesChangeNotifier, double>(
            selector: (_, provider) => provider.fromDirection(direction).price ?? 0,
            builder: (context, price, child) {
              return AmountTextField(
                value: Amount(price.ceil()),
                label: "Market Price (USD)",
              );
            }),
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Flexible(
                child: AmountInputField(
              value: tradeValues.quantity ?? Amount.zero(),
              hint: "e.g. 100 USD",
              label: "Quantity (USD)",
              onChanged: (value) {
                Amount quantity = Amount.zero();
                try {
                  if (value.isNotEmpty) {
                    quantity = Amount.parseAmount(value);
                  }

                  context.read<TradeValuesChangeNotifier>().updateQuantity(direction, quantity);
                } on Exception {
                  context
                      .read<TradeValuesChangeNotifier>()
                      .updateQuantity(direction, Amount.zero());
                }
                _formKey.currentState?.validate();
              },
              validator: (value) {
                // TODO(bonomat): we need new checks here. For now, YOLO
                // Amount quantity = Amount.parseAmount(value);
                //
                // // TODO(holzeis): fetch min amount to trade from coordinator
                // if (quantity.toInt < 1) {
                //   return "Min quantity to trade is 1";
                // }
                //
                // Amount margin = tradeValues.margin!;
                //
                // int? optCounterPartyMargin =
                //     provider.counterpartyMargin(direction, coordinatorLeverage);
                // if (optCounterPartyMargin == null) {
                //   return "Counterparty margin not available";
                // }
                // int counterpartyMargin = optCounterPartyMargin;
                //
                // int usableCounterpartyMarginInPosition = positionChangeNotifier
                //     .coordinatorMarginUsableForTrade(Leverage(coordinatorLeverage), direction)
                //     .sats;
                //
                // // This condition has to stay as the first thing to check, so we reset showing the info
                // if (counterpartyMargin >
                //     counterpartyUsableBalance + usableCounterpartyMarginInPosition) {
                //   setState(() => showCapacityInfo = true);
                //
                //   return "Insufficient capacity";
                // } else if (showCapacityInfo) {
                //   setState(() => showCapacityInfo = true);
                // }
                //
                // Amount fee = provider.orderMatchingFee(direction) ?? Amount.zero();
                // if (usableBalance < margin.sats + fee.sats) {
                //   return "Insufficient balance";
                // }
                //
                // if (margin.sats > maxMargin) {
                //   return "Max margin is $maxMargin";
                // }
                // if (margin.sats < minTradeMargin.sats) {
                //   return "Min margin is ${minTradeMargin.sats}";
                // }

                showCapacityInfo = false;
                return null;
              },
            )),
            const SizedBox(
              width: 10,
            ),
            Flexible(
                child: Selector<TradeValuesChangeNotifier, Amount>(
                    selector: (_, provider) =>
                        provider.fromDirection(direction).margin ?? Amount.zero(),
                    builder: (context, margin, child) {
                      return AmountTextField(
                        value: margin,
                        label: "Margin (sats)",
                      );
                    })),
            if (showCapacityInfo)
              ModalBottomSheetInfo(
                  closeButtonText: "Back to order",
                  child: Text(
                      "Your channel capacity is limited to ${formatSats(channelCapacity)} sats."
                      "In order to trade with higher margin you have to reduce your balance or create a bigger channel."
                      "\n\nYour current usable balance is ${formatSats(Amount(usableBalance))}.\n"
                      "Leaving your counterparty a possible margin of ${formatSats(Amount(channelCapacity.sats - usableBalance))}"))
          ],
        ),
        LeverageSlider(
            initialValue: positionLeverage ??
                context
                    .read<TradeValuesChangeNotifier>()
                    .fromDirection(direction)
                    .leverage
                    .leverage,
            isActive: !hasPosition,
            onLeverageChanged: (value) {
              context.read<TradeValuesChangeNotifier>().updateLeverage(direction, Leverage(value));
            }),
        Row(
          children: [
            Selector<TradeValuesChangeNotifier, double>(
                selector: (_, provider) =>
                    provider.fromDirection(direction).liquidationPrice ?? 0.0,
                builder: (context, liquidationPrice, child) {
                  return ValueDataRow(
                      type: ValueType.fiat, value: liquidationPrice, label: "Liquidation:");
                }),
            const SizedBox(width: 55),
            Selector<TradeValuesChangeNotifier, Amount>(
                selector: (_, provider) => provider.orderMatchingFee(direction) ?? Amount.zero(),
                builder: (context, fee, child) {
                  return ValueDataRow(type: ValueType.amount, value: fee, label: "Fee:");
                }),
          ],
        )
      ],
    );
  }
}
