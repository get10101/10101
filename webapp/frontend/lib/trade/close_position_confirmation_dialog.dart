import 'package:flutter/material.dart';
import 'package:get_10101/common/contract_symbol_icon.dart';
import 'package:get_10101/common/direction.dart';
import 'package:get_10101/common/model.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/common/theme.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/trade/new_order_service.dart';
import 'package:get_10101/services/quote_service.dart';

class TradeConfirmationDialog extends StatelessWidget {
  final Direction direction;
  final Function() onConfirmation;
  final BestQuote? bestQuote;
  final Amount? pnl;
  final Amount? fee;
  final Amount? payout;
  final Leverage leverage;
  final Usd quantity;

  const TradeConfirmationDialog(
      {super.key,
      required this.direction,
      required this.onConfirmation,
      required this.bestQuote,
      required this.pnl,
      required this.fee,
      required this.payout,
      required this.leverage,
      required this.quantity});

  @override
  Widget build(BuildContext context) {
    final messenger = ScaffoldMessenger.of(context);
    TenTenOneTheme tradeTheme = Theme.of(context).extension<TenTenOneTheme>()!;

    TextStyle dataRowStyle = const TextStyle(fontSize: 14);

    Price? price = bestQuote?.bid;
    if (direction == Direction.short) {
      price = bestQuote?.ask;
    }

    Color color = direction == Direction.long ? tradeTheme.buy : tradeTheme.sell;

    return Dialog(
      child: Padding(
        padding: const EdgeInsets.all(8.0),
        child: SizedBox(
          width: 340,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Container(
                  padding: const EdgeInsets.all(20),
                  child: Column(
                    children: [
                      const ContractSymbolIcon(),
                      Padding(
                        padding: const EdgeInsets.all(8.0),
                        child: Text("Market ${direction.nameU}",
                            style:
                                TextStyle(fontWeight: FontWeight.bold, fontSize: 17, color: color)),
                      ),
                      Center(
                        child: Container(
                          padding: const EdgeInsets.symmetric(vertical: 10),
                          child: Column(
                            children: [
                              Wrap(
                                runSpacing: 10,
                                children: [
                                  ValueDataRow(
                                      type: ValueType.fiat,
                                      value: price?.asDouble ?? 0.0,
                                      label: 'Latest Market Price'),
                                  ValueDataRow(
                                      type: ValueType.amount,
                                      value: pnl,
                                      label: 'Unrealized P/L',
                                      valueTextStyle: dataRowStyle.apply(
                                          color: pnl != null
                                              ? pnl!.sats.isNegative
                                                  ? tradeTheme.loss
                                                  : tradeTheme.profit
                                              : tradeTheme.disabled)),
                                  ValueDataRow(
                                    type: ValueType.amount,
                                    value: fee,
                                    label: "Fee estimate",
                                  ),
                                  ValueDataRow(
                                      type: ValueType.amount,
                                      value: payout,
                                      label: "Payout estimate",
                                      valueTextStyle: TextStyle(
                                          fontSize: dataRowStyle.fontSize,
                                          fontWeight: FontWeight.bold)),
                                ],
                              ),
                            ],
                          ),
                        ),
                      ),
                      Padding(
                        padding: const EdgeInsets.only(top: 20.0),
                        child: RichText(
                            textAlign: TextAlign.justify,
                            text: TextSpan(
                                text:
                                    'By confirming, a closing market order will be created. Once the order is matched your position will be closed.',
                                style: DefaultTextStyle.of(context).style)),
                      ),
                      Padding(
                        padding: const EdgeInsets.only(top: 20.0),
                        child: Row(
                          crossAxisAlignment: CrossAxisAlignment.center,
                          mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                          children: [
                            ElevatedButton(
                              onPressed: () {
                                Navigator.pop(context);
                              },
                              style: ElevatedButton.styleFrom(
                                  backgroundColor: Colors.grey, fixedSize: const Size(100, 20)),
                              child: const Text('Cancel'),
                            ),
                            ElevatedButton(
                              onPressed: () async {
                                await NewOrderService.postNewOrder(
                                        leverage, quantity, direction == Direction.long.opposite())
                                    .then((orderId) {
                                  showSnackBar(
                                      messenger, "Closing order created. Order id: $orderId.");
                                  Navigator.pop(context);
                                }).catchError((error) {
                                  showSnackBar(messenger, "Failed creating closing order: $error.");
                                });
                              },
                              style: ElevatedButton.styleFrom(fixedSize: const Size(100, 20)),
                              child: const Text('Accept'),
                            ),
                          ],
                        ),
                      ),
                    ],
                  ))
            ],
          ),
        ),
      ),
    );
  }
}
