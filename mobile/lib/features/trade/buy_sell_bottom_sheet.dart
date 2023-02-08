import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/leverage_slider.dart';
import 'package:get_10101/features/trade/trade_tabs.dart';
import 'package:get_10101/features/trade/trade_theme.dart';

enum Direction { buy, sell }

class PreservedOrderValues {
  double leverage;
  double quantity;
  double margin;

  PreservedOrderValues(this.leverage, this.quantity, this.margin);

  static PreservedOrderValues initial() => PreservedOrderValues(2, 0, 0);
}

class BuySellBottomSheet extends StatefulWidget {
  const BuySellBottomSheet({required this.direction, super.key});
  final Direction direction;

  @override
  State<BuySellBottomSheet> createState() => _BuySellBottomSheetState();
}

class _BuySellBottomSheetState extends State<BuySellBottomSheet> {
  // TODO: Move these into ChangeNotifier and potentially change the state handling.
  PreservedOrderValues buyValues = PreservedOrderValues.initial();
  PreservedOrderValues sellValues = PreservedOrderValues.initial();

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(20),
      child: TradeTabs(
        tabBarPadding: const EdgeInsets.only(bottom: 10.0),
        tabs: const ["Buy", "Sell"],
        tabBarViewChildren: [
          BuySellContents(
            leverage: buyValues.leverage,
            direction: Direction.buy,
            leverageChanged: (leverage) {
              setState(() {
                buyValues.leverage = leverage;
              });
            },
          ),
          BuySellContents(
            leverage: sellValues.leverage,
            direction: Direction.sell,
            leverageChanged: (leverage) {
              setState(() {
                sellValues.leverage = leverage;
              });
            },
          ),
        ],
        selectedIndex: widget.direction == Direction.buy ? 0 : 1,
        topRightWidget: Row(
          children: [
            const Text(
              "Market Order",
              style: TextStyle(color: Colors.grey),
            ),
            IconButton(
                onPressed: () {
                  showModalBottomSheet<void>(
                    shape: const RoundedRectangleBorder(
                      borderRadius: BorderRadius.vertical(
                        top: Radius.circular(20),
                      ),
                    ),
                    clipBehavior: Clip.antiAliasWithSaveLayer,
                    useRootNavigator: true,
                    context: context,
                    builder: (BuildContext context) {
                      return Container(
                        height: 300,
                        padding: const EdgeInsets.all(20.0),
                        child: Column(
                          mainAxisAlignment: MainAxisAlignment.spaceBetween,
                          children: [
                            // TODO: Add link to FAQ
                            const Text(
                                "For the beta phase only market orders are enabled in the 10101 app.\n\n"
                                "Market orders are executed at the best market price. \n\nPlease note that the displayed "
                                "price is the best market price at the time but due to fast market "
                                "movements the market price for order fulfillment can be slightly different."),
                            ElevatedButton(
                                onPressed: () => Navigator.pop(context),
                                child: const Text("Back to order..."))
                          ],
                        ),
                      );
                    },
                  );
                },
                icon: Icon(
                  Icons.info,
                  color: Theme.of(context).colorScheme.primary,
                ))
          ],
        ),
      ),
    );
  }
}

class BuySellContents extends StatelessWidget {
  const BuySellContents(
      {required this.leverageChanged, required this.direction, required this.leverage, super.key});

  final Direction direction;
  final Function(double) leverageChanged;
  final double leverage;

  @override
  Widget build(BuildContext context) {
    TradeTheme tradeTheme = Theme.of(context).extension<TradeTheme>()!;

    String label = direction == Direction.buy ? "Buy" : "Sell";
    Color color = direction == Direction.buy ? tradeTheme.buy : tradeTheme.sell;

    return Column(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      crossAxisAlignment: CrossAxisAlignment.center,
      mainAxisSize: MainAxisSize.min,
      children: [
        Wrap(
          runSpacing: 15,
          children: [
            Row(
              children: const [
                Flexible(child: Text("Available Balance:")),
                Flexible(child: Text("2,000 sats"))
              ],
            ),
            TextFormField(
              controller: TextEditingController()..text = '19,900.0',
              enabled: false,
              decoration: const InputDecoration(
                border: OutlineInputBorder(),
                hintText: "Price",
                labelText: "Market Price",
              ),
            ),
            Row(
              children: [
                Flexible(
                  child: TextFormField(
                    decoration: const InputDecoration(
                      border: OutlineInputBorder(),
                      hintText: "e.g. 100 USD",
                      labelText: "Quantity",
                    ),
                  ),
                ),
                const SizedBox(
                  width: 10,
                ),
                Flexible(
                  child: TextFormField(
                    decoration: const InputDecoration(
                      border: OutlineInputBorder(),
                      hintText: "e.g. 2,000 sats",
                      labelText: "Margin",
                    ),
                  ),
                ),
              ],
            ),
            LeverageSlider(
                initialValue: leverage,
                onLeverageChanged: (value) {
                  leverageChanged(value);
                })
          ],
        ),
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
    );
  }
}
