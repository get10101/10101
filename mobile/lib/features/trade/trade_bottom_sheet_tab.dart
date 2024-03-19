import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/amount_text_field.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/application/lsp_change_notifier.dart';
import 'package:get_10101/common/dlc_channel_change_notifier.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/channel_configuration.dart';
import 'package:get_10101/features/trade/domain/channel_opening_params.dart';
import 'package:get_10101/ffi.dart' as rust;
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
import 'package:get_10101/logger/logger.dart';
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
  late final TradeValuesChangeNotifier tradeValueChangeNotifier;
  late final LspChangeNotifier lspChangeNotifier;
  late final PositionChangeNotifier positionChangeNotifier;
  late final TradeValuesService tradeValuesService;

  final _formKey = GlobalKey<FormState>();

  bool showCapacityInfo = false;

  bool marginInputFieldEnabled = false;
  bool quantityInputFieldEnabled = true;

  @override
  void initState() {
    tradeValueChangeNotifier = context.read<TradeValuesChangeNotifier>();
    lspChangeNotifier = context.read<LspChangeNotifier>();
    positionChangeNotifier = context.read<PositionChangeNotifier>();
    tradeValuesService = tradeValueChangeNotifier.tradeValuesService;

    context.read<DlcChannelChangeNotifier>().refreshDlcChannels();

    super.initState();
  }

  @override
  void dispose() {
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    TradeTheme tradeTheme = Theme.of(context).extension<TradeTheme>()!;
    DlcChannelChangeNotifier dlcChannelChangeNotifier = context.watch<DlcChannelChangeNotifier>();

    Direction direction = widget.direction;
    String label = direction == Direction.long ? "Buy" : "Sell";
    Color color = direction == Direction.long ? tradeTheme.buy : tradeTheme.sell;

    final channelInfoService = lspChangeNotifier.channelInfoService;
    final channelTradeConstraints = channelInfoService.getTradeConstraints();

    final hasChannel = dlcChannelChangeNotifier.hasDlcChannel();

    return Form(
      key: _formKey,
      child: Column(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        crossAxisAlignment: CrossAxisAlignment.center,
        mainAxisSize: MainAxisSize.min,
        children: [
          Center(
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: <Widget>[
                buildChildren(
                    direction, channelTradeConstraints, context, channelInfoService, _formKey)
              ],
            ),
          ),
          Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              ElevatedButton(
                  key: widget.buttonKey,
                  onPressed: () {
                    if (_formKey.currentState!.validate()) {
                      final submitOrderChangeNotifier = context.read<SubmitOrderChangeNotifier>();

                      final tradeAction = hasChannel ? TradeAction.trade : TradeAction.openChannel;

                      final tradeValues =
                          context.read<TradeValuesChangeNotifier>().fromDirection(direction);

                      switch (tradeAction) {
                        case TradeAction.openChannel:
                          {
                            channelConfiguration(
                              context: context,
                              tradeValues: tradeValues,
                              onConfirmation: (ChannelOpeningParams channelOpeningParams) {
                                tradeBottomSheetConfirmation(
                                  context: context,
                                  direction: direction,
                                  tradeAction: tradeAction,
                                  onConfirmation: () => onConfirmation(
                                      submitOrderChangeNotifier, tradeValues, channelOpeningParams),
                                  channelOpeningParams: channelOpeningParams,
                                  tradeValues: tradeValues,
                                );
                              },
                            );
                            break;
                          }
                        case TradeAction.trade:
                        case TradeAction.closePosition:
                          logger.i("Opening dialog with: ${tradeValues.margin}");
                          tradeBottomSheetConfirmation(
                            context: context,
                            direction: direction,
                            tradeAction: tradeAction,
                            onConfirmation: () =>
                                onConfirmation(submitOrderChangeNotifier, tradeValues, null),
                            channelOpeningParams: null,
                            tradeValues: tradeValues,
                          );
                      }
                    }
                  },
                  style: ElevatedButton.styleFrom(
                      backgroundColor: color, minimumSize: const Size.fromHeight(50)),
                  child: Text(
                    label,
                    style: const TextStyle(color: Colors.white),
                  )),
            ],
          )
        ],
      ),
    );
  }

  void onConfirmation(SubmitOrderChangeNotifier submitOrderChangeNotifier, TradeValues tradeValues,
      ChannelOpeningParams? channelOpeningParams) {
    submitOrderChangeNotifier.submitPendingOrder(tradeValues, PositionAction.open,
        channelOpeningParams: channelOpeningParams);

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
  }

  Wrap buildChildren(Direction direction, rust.TradeConstraints channelTradeConstraints,
      BuildContext context, ChannelInfoService channelInfoService, GlobalKey<FormState> formKey) {
    var tradeValuesChangeNotifier = context.read<TradeValuesChangeNotifier>();
    final tradeValues = tradeValuesChangeNotifier.fromDirection(direction);

    bool hasPosition = positionChangeNotifier.positions.containsKey(contractSymbol);

    double? positionLeverage;
    if (hasPosition) {
      final position = context.read<PositionChangeNotifier>().positions[contractSymbol];
      positionLeverage = position!.leverage.leverage;
    }

    int usableBalance = channelTradeConstraints.maxLocalMarginSats;

    // We compute the max quantity based on the margin needed for the counterparty and how much he has available.
    // TODO: this won't be updated but I guess it's ok because the price won't move too much.
    final double priceForMaxQuantity = tradeValues.price ?? 0.0;
    double maxQuantity = (channelTradeConstraints.maxCounterpartyMarginSats / 100000000) *
        priceForMaxQuantity *
        channelTradeConstraints.coordinatorLeverage;

    return Wrap(
      runSpacing: 12,
      children: [
        Padding(
          padding: const EdgeInsets.only(bottom: 10),
          child: Row(
            children: [
              const Flexible(child: Text("Balance:")),
              const SizedBox(width: 5),
              Flexible(child: AmountText(amount: Amount(usableBalance))),
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
                child: GestureDetector(
              onTap: () {
                setState(() {
                  marginInputFieldEnabled = false;
                  quantityInputFieldEnabled = true;
                  tradeValuesChangeNotifier.updateIsMargin(direction, false);
                });
              },
              child: AmountInputField(
                controller: tradeValues.quantityController,
                enabled: quantityInputFieldEnabled,
                hint: "e.g. 100 USD",
                label: "Quantity (USD)",
                onChanged: (value) {
                  Usd innerQuantity = Usd.zero();
                  try {
                    if (value.isNotEmpty) {
                      innerQuantity = Usd.parseString(value);
                    }
                    tradeValuesChangeNotifier.updateQuantity(direction, innerQuantity);
                  } on Exception {
                    tradeValuesChangeNotifier.updateQuantity(direction, Usd.zero());
                  }
                  _formKey.currentState?.validate();
                },
                validator: (ignored) {
                  Usd quantity = tradeValues.quantity ?? Usd.zero();
                  Amount margin = tradeValues.margin ?? Amount.zero();

                  if (quantity.asDouble() < channelTradeConstraints.minQuantity.toDouble()) {
                    return "Min quantity is ${channelTradeConstraints.minQuantity}";
                  }

                  if (quantity.asDouble() > maxQuantity.toDouble()) {
                    setState(() => showCapacityInfo = true);
                    return "Max quantity is ${maxQuantity.toInt()}";
                  }

                  double coordinatorLeverage = channelTradeConstraints.coordinatorLeverage;

                  int? optCounterPartyMargin = tradeValueChangeNotifier.counterpartyMargin(
                      direction,
                      coordinatorLeverage,
                      tradeValues.price?.toDouble() ?? 0.0,
                      quantity);
                  if (optCounterPartyMargin == null) {
                    return "Counterparty margin not available";
                  }
                  int neededCounterpartyMarginSats = optCounterPartyMargin;

                  // This condition has to stay as the first thing to check, so we reset showing the info
                  int maxCounterpartyMarginSats = channelTradeConstraints.maxCounterpartyMarginSats;
                  int maxLocalMarginSats = channelTradeConstraints.maxLocalMarginSats;

                  // First we check if we have enough money, then we check if counterparty would have enough money
                  Amount fee =
                      tradeValueChangeNotifier.orderMatchingFee(direction) ?? Amount.zero();

                  int neededLocalMarginSats = margin.sats + fee.sats;

                  if (neededLocalMarginSats > maxLocalMarginSats) {
                    setState(() => showCapacityInfo = true);
                    return "Insufficient balance";
                  }

                  if (neededCounterpartyMarginSats > maxCounterpartyMarginSats) {
                    setState(() => showCapacityInfo = true);
                    return "Counterparty has insufficient balance";
                  }

                  setState(() {
                    showCapacityInfo = false;
                  });
                  return null;
                },
              ),
            )),
            const SizedBox(
              width: 10,
            ),
            Flexible(
                child: GestureDetector(
              onTap: () {
                setState(() {
                  marginInputFieldEnabled = true;
                  quantityInputFieldEnabled = false;
                  tradeValuesChangeNotifier.updateIsMargin(direction, true);
                });
              },
              child: AmountInputField(
                  enabled: marginInputFieldEnabled,
                  controller: tradeValues.marginController,
                  label: "Margin (sats)",
                  onChanged: (newMarginValue) {
                    Amount newMargin = Amount.zero();
                    try {
                      if (newMarginValue.isNotEmpty) {
                        newMargin = Amount.parseAmount(newMarginValue);
                      }
                      tradeValuesChangeNotifier.updateMargin(direction, newMargin);
                    } on Exception {
                      tradeValues.updateMargin(Amount.zero());
                    }
                    _formKey.currentState?.validate();
                  }),
            )),
          ],
        ),
        LeverageSlider(
            initialValue: positionLeverage ?? tradeValues.leverage.leverage,
            isActive: !hasPosition,
            onLeverageChanged: (value) {
              tradeValues.updateLeverage(Leverage(value));
              formKey.currentState!.validate();
            }),
        Row(
          children: [
            ValueDataRow(
                type: ValueType.fiat, value: tradeValues.liquidationPrice, label: "Liquidation:"),
            const SizedBox(width: 55),
            ValueDataRow(type: ValueType.amount, value: tradeValues.fee, label: "Fee:"),
          ],
        )
      ],
    );
  }
}
