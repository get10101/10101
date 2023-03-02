import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/double_text_input_form_field.dart';
import 'package:get_10101/common/fiat_text.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/features/trade/leverage_slider.dart';
import 'package:get_10101/features/trade/trade_bottom_sheet_confirmation.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:get_10101/features/wallet/domain/wallet_info.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:provider/provider.dart';

class TradeBottomSheetTab extends StatelessWidget {
  final Direction direction;
  final Key buttonKey;

  const TradeBottomSheetTab({required this.direction, super.key, required this.buttonKey});

  @override
  Widget build(BuildContext context) {
    TradeTheme tradeTheme = Theme.of(context).extension<TradeTheme>()!;

    WalletInfo walletInfo = context.watch<WalletChangeNotifier>().walletInfo;

    String label = direction == Direction.long ? "Buy" : "Sell";
    Color color = direction == Direction.long ? tradeTheme.buy : tradeTheme.sell;

    TradeValuesChangeNotifier provider = context.read<TradeValuesChangeNotifier>();

    TextEditingController marginController =
        TextEditingController(text: provider.fromDirection(direction).margin.toString());
    TextEditingController quantityController =
        TextEditingController(text: provider.fromDirection(direction).quantity.toString());
    TextEditingController priceController =
        TextEditingController(text: provider.fromDirection(direction).price.toString());

    return Column(
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
                  const Flexible(child: Text("Available Balance:")),
                  const SizedBox(width: 5),
                  Flexible(child: AmountText(amount: walletInfo.balances.lightning))
                ],
              ),
            ),
            DoubleTextInputFormField(
              controller: priceController,
              enabled: false,
              label: "Market Price",
            ),
            Row(
              children: [
                Flexible(
                  child: Selector<TradeValuesChangeNotifier, double>(
                    selector: (_, provider) => provider.fromDirection(direction).quantity,
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
                    selector: (_, provider) => provider.fromDirection(direction).margin,
                    builder: (context, margin, child) {
                      return AmountInputField(
                        value: margin,
                        hint: "e.g. 2,000 sats",
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
                      );
                    },
                  ),
                ),
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
                    selector: (_, provider) => provider.fromDirection(direction).liquidationPrice,
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
                key: buttonKey,
                onPressed: () {
                  tradeBottomSheetConfirmation(context: context, direction: direction);
                },
                style: ElevatedButton.styleFrom(
                    backgroundColor: color, minimumSize: const Size.fromHeight(50)),
                child: Text(label)),
          ],
        )
      ],
    );
  }
}
