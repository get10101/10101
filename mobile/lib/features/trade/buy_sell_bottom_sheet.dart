import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/trade_tabs.dart';
import 'package:get_10101/features/trade/trade_theme.dart';

enum Direction { buy, sell }

class BuySellBottomSheet extends StatelessWidget {
  const BuySellBottomSheet({required this.direction, super.key});

  final Direction direction;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(20),
      child: TradeTabs(
        tabs: const ["Buy", "Sell"],
        tabBarViewChildren: const [
          BuySellContents(direction: Direction.buy),
          BuySellContents(direction: Direction.sell)
        ],
        selectedIndex: direction == Direction.buy ? 0 : 1,
      ),
    );
  }
}

class BuySellContents extends StatelessWidget {
  const BuySellContents({required this.direction, super.key});

  final Direction direction;

  @override
  Widget build(BuildContext context) {
    TradeTheme tradeTheme = Theme.of(context).extension<TradeTheme>()!;

    String label = direction == Direction.buy ? "Buy" : "Sell";
    Color color = direction == Direction.buy ? tradeTheme.buy : tradeTheme.sell;

    return Container(
      padding: const EdgeInsets.only(top: 20),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Center(child: Text("$label Content")),
          const Spacer(),
          Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              ElevatedButton(
                  onPressed: () {},
                  style: ElevatedButton.styleFrom(
                      backgroundColor: color, minimumSize: const Size.fromHeight(50)),
                  child: Text(label)),
            ],
          )
        ],
      ),
    );
  }
}
