import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/trade/contract_symbol_icon.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/trade_values.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:get_10101/util/constants.dart';
import 'package:provider/provider.dart';
import 'package:slide_to_confirm/slide_to_confirm.dart';

tradeBottomSheetConfirmation(
    {required BuildContext context,
    required Direction direction,
    required Function() onConfirmation,
    bool close = false}) {
  final sliderKey = direction == Direction.long
      ? tradeScreenBottomSheetConfirmationSliderBuy
      : tradeScreenBottomSheetConfirmationSliderSell;

  final sliderButtonKey = direction == Direction.long
      ? tradeScreenBottomSheetConfirmationSliderButtonBuy
      : tradeScreenBottomSheetConfirmationSliderButtonSell;

  showModalBottomSheet<void>(
    shape: const RoundedRectangleBorder(
      borderRadius: BorderRadius.vertical(
        top: Radius.circular(20),
      ),
    ),
    clipBehavior: Clip.antiAlias,
    isScrollControlled: true,
    useRootNavigator: true,
    context: context,
    builder: (BuildContext context) {
      return SafeArea(
        child: Padding(
          // padding: MediaQuery.of(context).viewInsets,
          padding: EdgeInsets.only(bottom: MediaQuery.of(context).viewInsets.bottom),
          // the GestureDetector ensures that we can close the keyboard by tapping into the modal
          child: GestureDetector(
            onTap: () {
              FocusScopeNode currentFocus = FocusScope.of(context);

              if (!currentFocus.hasPrimaryFocus) {
                currentFocus.unfocus();
              }
            },
            child: SizedBox(
                height: 350,
                child: TradeBottomSheetConfirmation(
                  direction: direction,
                  sliderButtonKey: sliderButtonKey,
                  sliderKey: sliderKey,
                  onConfirmation: onConfirmation,
                  close: close,
                )),
          ),
        ),
      );
    },
  );
}

class TradeBottomSheetConfirmation extends StatelessWidget {
  final Direction direction;
  final Key sliderKey;
  final Key sliderButtonKey;
  final Function() onConfirmation;
  final bool close;

  const TradeBottomSheetConfirmation(
      {required this.direction,
      super.key,
      required this.sliderButtonKey,
      required this.sliderKey,
      required this.onConfirmation,
      required this.close});

  @override
  Widget build(BuildContext context) {
    TradeTheme tradeTheme = Theme.of(context).extension<TradeTheme>()!;
    Color color = direction == Direction.long ? tradeTheme.buy : tradeTheme.sell;

    TradeValues tradeValues =
        Provider.of<TradeValuesChangeNotifier>(context).fromDirection(direction);

    // Fallback to 0 if we can't get the fee or the margin
    Amount total = tradeValues.fee != null && tradeValues.margin != null
        ? Amount(tradeValues.fee!.sats + tradeValues.margin!.sats)
        : Amount(0);
    Amount pnl = Amount(0);
    if (context.read<PositionChangeNotifier>().positions.containsKey(ContractSymbol.btcusd)) {
      final position = context.read<PositionChangeNotifier>().positions[ContractSymbol.btcusd];
      pnl = position!.unrealizedPnl != null ? position.unrealizedPnl! : Amount(0);
    }

    DateTime now = DateTime.now().toUtc();
    TextStyle dataRowStyle = const TextStyle(fontSize: 14);

    return Container(
        padding: const EdgeInsets.all(20),
        child: Column(
          children: [
            const ContractSymbolIcon(),
            Text("Market ${direction.nameU}",
                style: TextStyle(fontWeight: FontWeight.bold, fontSize: 17, color: color)),
            Center(
              child: Container(
                padding: const EdgeInsets.symmetric(vertical: 10),
                child: Column(
                  children: [
                    Wrap(
                      runSpacing: 10,
                      children: [
                        if (!close)
                          ValueDataRow(
                              type: ValueType.date,
                              value: DateTime.utc(now.year, now.month, now.day + 2).toLocal(),
                              label: 'Expiry'),
                        close
                            ? ValueDataRow(
                                type: ValueType.fiat,
                                value: tradeValues.price ?? 0.0,
                                label: 'Market Price')
                            : ValueDataRow(
                                type: ValueType.amount, value: tradeValues.margin, label: 'Margin'),
                        close
                            ? ValueDataRow(
                                type: ValueType.amount,
                                value: pnl,
                                label: 'Unrealized P/L',
                                valueTextStyle: dataRowStyle.apply(
                                    color:
                                        pnl.sats.isNegative ? tradeTheme.loss : tradeTheme.profit))
                            : ValueDataRow(
                                type: ValueType.fiat,
                                value: tradeValues.liquidationPrice ?? 0.0,
                                label: 'Liquidation Price',
                              ),
                        ValueDataRow(
                          type: ValueType.amount,
                          value: tradeValues.fee ?? Amount.zero(),
                          label: "Fee estimate",
                        ),
                      ],
                    ),
                    !close ? const Divider() : const SizedBox(height: 0),
                    !close
                        ? ValueDataRow(type: ValueType.amount, value: total, label: "Total")
                        : const SizedBox(height: 0),
                  ],
                ),
              ),
            ),
            close
                ? RichText(
                    text: TextSpan(
                        text:
                            '\nBy confirming, a closing market order will be created. Once the order is matched your position will be closed.',
                        style: DefaultTextStyle.of(context).style))
                : RichText(
                    text: TextSpan(
                      text: 'By confirming a new order will be created. Once the order is matched ',
                      style: DefaultTextStyle.of(context).style,
                      children: <TextSpan>[
                        TextSpan(
                            text: formatSats(total),
                            style: const TextStyle(fontWeight: FontWeight.bold)),
                        const TextSpan(text: ' will be locked up in a Lightning channel!'),
                      ],
                    ),
                  ),
            const Spacer(),
            ConfirmationSlider(
              key: sliderKey,
              text: "Swipe to confirm ${direction.nameU}",
              textStyle: TextStyle(color: color),
              height: 40,
              foregroundColor: color,
              sliderButtonContent: Container(
                key: sliderButtonKey,
                child: const Icon(
                  Icons.chevron_right,
                  color: Colors.white,
                  size: 20,
                ),
              ),
              onConfirmation: onConfirmation,
            )
          ],
        ));
  }
}
