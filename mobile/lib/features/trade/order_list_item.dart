import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:intl/intl.dart';
import 'package:get_10101/features/trade/contract_symbol_icon.dart';
import 'package:get_10101/features/trade/domain/order.dart';

class OrderListItem extends StatelessWidget {
  const OrderListItem({super.key, required this.order});

  final Order order;

  @override
  Widget build(BuildContext context) {
    TradeTheme tradeTheme = Theme.of(context).extension<TradeTheme>()!;

    final formatter = NumberFormat();
    formatter.minimumFractionDigits = 2;
    formatter.maximumFractionDigits = 2;

    const double iconSize = 18;
    Icon statusIcon = switch (order.state) {
      OrderState.open => const Icon(
          Icons.pending,
          size: iconSize,
        ),
      OrderState.filling => const Icon(
          Icons.pending,
          size: iconSize,
        ),
      OrderState.filled => const Icon(Icons.check_circle, color: Colors.green, size: iconSize),
      OrderState.failed => const Icon(Icons.error, color: Colors.red, size: iconSize),
      OrderState.rejected => const Icon(Icons.error, color: Colors.red, size: iconSize),
    };

    return Column(
      children: [
        Card(
          margin: const EdgeInsets.all(0),
          elevation: 0,
          child: ListTile(
            leading: const Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                ContractSymbolIcon(
                  height: 20,
                  width: 20,
                  paddingUsd: EdgeInsets.only(left: 12.0),
                ),
              ],
            ),
            title: RichText(
              text: TextSpan(
                style: DefaultTextStyle.of(context).style,
                children: <TextSpan>[
                  TextSpan(text: "${order.leverage.formatted()} "),
                  TextSpan(
                      text: order.direction.nameU,
                      style: TextStyle(
                          color:
                              order.direction == Direction.long ? tradeTheme.buy : tradeTheme.sell,
                          fontWeight: FontWeight.bold)),
                  TextSpan(
                      text: " ${order.quantity} ",
                      style: const TextStyle(fontWeight: FontWeight.bold)),
                ],
              ),
            ),
            trailing: RichText(
              text: TextSpan(style: DefaultTextStyle.of(context).style, children: <InlineSpan>[
                WidgetSpan(alignment: PlaceholderAlignment.middle, child: statusIcon),
                TextSpan(
                    text: order.state == OrderState.filled && order.reason != OrderReason.manual
                        ? " ${order.reason.name}"
                        : " ${order.state.name}")
              ]),
            ),
            subtitle: RichText(
                textWidthBasis: TextWidthBasis.longestLine,
                text: TextSpan(style: DefaultTextStyle.of(context).style, children: <TextSpan>[
                  const TextSpan(text: "@ ", style: TextStyle(color: Colors.grey)),
                  TextSpan(
                      text: order.executionPrice != null
                          ? "${order.executionPrice!}"
                          : "Market Price")
                ])),
          ),
        ),
        const Divider(height: 0, thickness: 1, indent: 10, endIndent: 10)
      ],
    );
  }
}
