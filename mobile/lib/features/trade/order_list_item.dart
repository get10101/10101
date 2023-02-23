import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/trade_theme.dart';

import 'contract_symbol_icon.dart';
import 'domain/order.dart';

class OrderListItem extends StatelessWidget {
  const OrderListItem({super.key, required this.order});

  final Order order;

  @override
  Widget build(BuildContext context) {
    TradeTheme tradeTheme = Theme.of(context).extension<TradeTheme>()!;

    const double iconSize = 18;
    Icon statusIcon = () {
      switch (order.status) {
        case OrderState.open:
          return const Icon(
            Icons.pending,
            size: iconSize,
          );
        case OrderState.filled:
          return const Icon(Icons.check_circle, color: Colors.green, size: iconSize);
        case OrderState.failed:
          return const Icon(Icons.error, color: Colors.red, size: iconSize);
      }
    }();

    return Card(
      child: ListTile(
        leading: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: const [
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
                      color: order.direction == Direction.long ? tradeTheme.buy : tradeTheme.sell,
                      fontWeight: FontWeight.bold)),
              TextSpan(text: " ${order.quantity} ", style: const TextStyle(fontWeight: FontWeight.bold)),
              const TextSpan(text: "contracts")
            ],
          ),
        ),
        trailing: RichText(
          text: TextSpan(style: DefaultTextStyle.of(context).style, children: <InlineSpan>[
            WidgetSpan(alignment: PlaceholderAlignment.middle, child: statusIcon),
            TextSpan(text: " ${order.status.name}")
          ]),
        ),
        subtitle: RichText(
            textWidthBasis: TextWidthBasis.longestLine,
            text: TextSpan(style: DefaultTextStyle.of(context).style, children: <TextSpan>[
              const TextSpan(text: "@ ", style: TextStyle(color: Colors.grey)),
              TextSpan(text: "${order.executionPrice ?? "Market Price"}")
            ])),
      ),
    );
  }
}
