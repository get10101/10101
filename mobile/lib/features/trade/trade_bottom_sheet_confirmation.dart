import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/trade/btc_usd_trading_pair_image.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/trade_values.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';
import 'package:slide_to_confirm/slide_to_confirm.dart';

tradeBottomSheetConfirmation({required BuildContext context, required Direction direction}) {
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
            child: SizedBox(height: 350, child: TradeBottomSheetConfirmation(direction: direction)),
          ),
        ),
      );
    },
  );
}

class TradeBottomSheetConfirmation extends StatelessWidget {
  const TradeBottomSheetConfirmation({required this.direction, super.key});
  final Direction direction;

  @override
  Widget build(BuildContext context) {
    TradeTheme tradeTheme = Theme.of(context).extension<TradeTheme>()!;
    Color color = direction == Direction.buy ? tradeTheme.buy : tradeTheme.sell;

    TradeValues tradeValues =
        Provider.of<TradeValuesChangeNotifier>(context).fromDirection(direction);

    Amount total = Amount(tradeValues.fee.sats + tradeValues.margin.sats);

    return Container(
        padding: const EdgeInsets.all(20),
        child: Column(
          children: [
            const BtcUsdTradingPairImage(),
            Text("Market ${direction.nameU}",
                style: TextStyle(fontWeight: FontWeight.bold, fontSize: 17, color: color)),
            Center(
              child: Container(
                padding: const EdgeInsets.symmetric(vertical: 10),
                width: 250,
                child: Column(
                  children: [
                    Wrap(
                      runSpacing: 10,
                      children: [
                        ValueDataRow(
                            type: ValueType.amount, value: tradeValues.margin, label: 'Margin'),
                        ValueDataRow(
                          type: ValueType.fiat,
                          value: tradeValues.liquidationPrice,
                          label: 'Liquidation Price',
                        ),
                        ValueDataRow(
                            type: ValueType.percentage,
                            value: tradeValues.fundingRate,
                            label: "Funding Rate"),
                        ValueDataRow(type: ValueType.amount, value: tradeValues.fee, label: "Fee"),
                      ],
                    ),
                    const Divider(),
                    ValueDataRow(type: ValueType.amount, value: total, label: "Total")
                  ],
                ),
              ),
            ),
            RichText(
              text: TextSpan(
                text:
                    'By confirming a new tradeValues will be created. Once the tradeValues is matched ',
                style: DefaultTextStyle.of(context).style,
                children: <TextSpan>[
                  TextSpan(
                      text: formatAmount(AmountDenomination.satoshi, total),
                      style: const TextStyle(fontWeight: FontWeight.bold)),
                  const TextSpan(text: ' will be locked up in a Lightning channel!'),
                ],
              ),
            ),
            const Spacer(),
            ConfirmationSlider(
              text: "Swipe to confirm ${direction.nameU}",
              textStyle: TextStyle(color: color),
              height: 40,
              foregroundColor: color,
              sliderButtonContent: const Icon(
                Icons.chevron_right,
                color: Colors.white,
                size: 20,
              ),
              onConfirmation: () async {
                context.read<SubmitOrderChangeNotifier>().submitPendingOrder(tradeValues);

                // TODO: Explore if it would be easier / better handle the popups as routes
                // Pop twice to navigate back to the trade screen.
                GoRouter.of(context).pop();
                GoRouter.of(context).pop();
              },
            )
          ],
        ));
  }
}
