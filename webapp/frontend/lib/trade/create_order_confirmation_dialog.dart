import 'package:flutter/material.dart';
import 'package:get_10101/common/contract_symbol_icon.dart';
import 'package:get_10101/common/direction.dart';
import 'package:get_10101/common/model.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/common/theme.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/services/new_order_service.dart';
import 'package:get_10101/services/quote_service.dart';

class CreateOrderConfirmationDialog extends StatelessWidget {
  final Direction direction;
  final Function() onConfirmation;
  final Function() onCancel;
  final BestQuote? bestQuote;
  final Amount? fee;
  final Leverage leverage;
  final Usd quantity;

  const CreateOrderConfirmationDialog(
      {super.key,
      required this.direction,
      required this.onConfirmation,
      required this.onCancel,
      required this.bestQuote,
      required this.fee,
      required this.leverage,
      required this.quantity});

  @override
  Widget build(BuildContext context) {
    final messenger = ScaffoldMessenger.of(context);
    TenTenOneTheme tradeTheme = Theme.of(context).extension<TenTenOneTheme>()!;

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
                                      type: ValueType.contracts,
                                      value: quantity.formatted(),
                                      label: 'Quantity'),
                                  //
                                  ValueDataRow(
                                      type: ValueType.text,
                                      value: leverage.formatted(),
                                      label: 'Leverage'),
                                  //
                                  ValueDataRow(
                                      type: ValueType.fiat,
                                      value: price?.asDouble ?? 0.0,
                                      label: 'Latest Market Price'),
                                  //
                                  ValueDataRow(
                                    type: ValueType.amount,
                                    value: fee ?? Amount.zero(),
                                    label: "Fee estimate",
                                  ),
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
                                    'By confirming, a market order will be created. Once the order is matched your position will be updated.',
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
                                onCancel();
                                Navigator.pop(context);
                              },
                              style: ElevatedButton.styleFrom(
                                  backgroundColor: Colors.grey, fixedSize: const Size(100, 20)),
                              child: const Text('Cancel'),
                            ),
                            ElevatedButton(
                              onPressed: () async {
                                await NewOrderService.postNewOrder(
                                        leverage, quantity, direction == Direction.long)
                                    .then((orderId) {
                                  showSnackBar(
                                      messenger, "Market order created. Order id: $orderId.");
                                  Navigator.pop(context);
                                }).catchError((error) {
                                  showSnackBar(messenger, "Failed creating market order: $error.");
                                }).whenComplete(onConfirmation);
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
