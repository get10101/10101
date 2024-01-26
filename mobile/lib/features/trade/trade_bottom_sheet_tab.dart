import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/amount_text_field.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/application/lsp_change_notifier.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/ffi.dart' as rust;
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

    Direction direction = widget.direction;
    String label = direction == Direction.long ? "Buy" : "Sell";
    Color color = direction == Direction.long ? tradeTheme.buy : tradeTheme.sell;

    final channelInfoService = lspChangeNotifier.channelInfoService;
    final channelTradeConstraints = channelInfoService.getTradeConstraints();

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

  Wrap buildChildren(Direction direction, rust.TradeConstraints channelTradeConstraints,
      BuildContext context, ChannelInfoService channelInfoService, GlobalKey<FormState> formKey) {
    final tradeValues = context.read<TradeValuesChangeNotifier>().fromDirection(direction);

    bool hasPosition = positionChangeNotifier.positions.containsKey(contractSymbol);

    double? positionLeverage;
    if (hasPosition) {
      final position = context.read<PositionChangeNotifier>().positions[contractSymbol];
      positionLeverage = position!.leverage.leverage;
    }

    int usableBalance = channelTradeConstraints.maxLocalMarginSats;
    bool isChannelBalance = channelTradeConstraints.isChannelBalance;

    // We compute the max quantity based on the margin needed for the counterparty and how much he has available.
    double price = tradeValues.price ?? 0.0;
    double maxQuantity = (channelTradeConstraints.maxCounterpartyMarginSats / 100000000) *
        price *
        channelTradeConstraints.coordinatorLeverage;

    String text =
        "The usable balance of ${formatSats(Amount(usableBalance))} are your on-chain funds. If you need more, you can always deposit more into you wallet. "
        "\nWith your current balance, the maximum you can trade is ${formatUsd(Usd(maxQuantity.toInt()))}";
    if (isChannelBalance) {
      text =
          "The usable balance of ${formatSats(Amount(usableBalance))} are your off-chain funds. At the moment you can't add more than this as we do not support splicing. \n"
          "If you want to trade more than this, you will need to close the channel and open a bigger one. "
          "\nWith your current balance, the maximum you can trade is ${formatUsd(Usd(maxQuantity.toInt()))}";
    }

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
              const SizedBox(
                width: 5,
              ),
              ModalBottomSheetInfo(
                closeButtonText: "Back to order",
                infoButtonPadding: const EdgeInsets.symmetric(horizontal: 8.0),
                child: Text(text),
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
                Amount quantity = Amount.parseAmount(value);

                if (quantity.toInt < channelTradeConstraints.minQuantity) {
                  return "Min quantity is ${channelTradeConstraints.minQuantity}";
                }

                if (quantity.toInt > maxQuantity) {
                  setState(() => showCapacityInfo = true);
                  return "Max quantity is ${maxQuantity.toInt()}";
                }

                double coordinatorLeverage = channelTradeConstraints.coordinatorLeverage;

                int? optCounterPartyMargin =
                    provider.counterpartyMargin(direction, coordinatorLeverage);
                if (optCounterPartyMargin == null) {
                  return "Counterparty margin not available";
                }
                int neededCounterpartyMarginSats = optCounterPartyMargin;

                // This condition has to stay as the first thing to check, so we reset showing the info
                int maxCounterpartyMarginSats = channelTradeConstraints.maxCounterpartyMarginSats;
                int maxLocalMarginSats = channelTradeConstraints.maxLocalMarginSats;

                // First we check if we have enough money, then we check if counterparty would have enough money
                Amount fee = provider.orderMatchingFee(direction) ?? Amount.zero();

                Amount margin = tradeValues.margin!;
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
                        suffixIcon: showCapacityInfo
                            ? ModalBottomSheetInfo(
                                closeButtonText: "Back to order",
                                child: Text(
                                    "The max amount you can trade depends on your balance, your counterparty's balance and your leverage: \n\n"
                                    "- Your max margin is ${formatSats(Amount(usableBalance))}\n"
                                    "- Counterparty max margin is ${formatSats(Amount(channelTradeConstraints.maxLocalMarginSats))}\n"
                                    "- This results in a max amount of ${formatUsd(Usd(maxQuantity.toInt()))} with your current leverage of ${tradeValues.leverage.formatted()}"))
                            : null,
                      );
                    })),
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
              // When the slider changes, we validate the whole form.
              formKey.currentState!.validate();
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
