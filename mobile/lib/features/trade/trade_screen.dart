import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/btc_usd_trading_pair_image.dart';
import 'package:get_10101/features/trade/buy_sell_bottom_sheet.dart';
import 'package:get_10101/features/trade/candlestick_chart.dart';
import 'package:get_10101/features/trade/trade_tabs.dart';
import 'package:get_10101/features/trade/trade_theme.dart';

class TradeScreen extends StatefulWidget {
  static const route = "/trade";
  static const label = "Trade";

  const TradeScreen({Key? key}) : super(key: key);

  @override
  State<TradeScreen> createState() => _TradeScreenState();
}

class _TradeScreenState extends State<TradeScreen> {
  @override
  Widget build(BuildContext context) {
    TradeTheme tradeTheme = Theme.of(context).extension<TradeTheme>()!;

    List<String> orders = List<String>.generate(100, (i) => 'Order $i');
    List<String> positions = List<String>.generate(100, (i) => 'Position $i');

    const RoundedRectangleBorder tradeButtonShape = RoundedRectangleBorder(
      borderRadius: BorderRadius.all(
        Radius.circular(8),
      ),
    );

    const double tradeButtonWidth = 100.0;

    return Scaffold(
        body: Container(
          padding: const EdgeInsets.only(left: 15, right: 15),
          child: Column(
            children: [
              Row(
                children: const [BtcUsdTradingPairImage(), Text("BTC/USD")],
              ),
              Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: const [CandlestickChart()],
              ),
              Expanded(
                child: TradeTabs(
                  tabs: const [
                    "Positions",
                    "Orders",
                  ],
                  tabBarViewChildren: [
                    ListView.builder(
                      shrinkWrap: true,
                      physics: const ClampingScrollPhysics(),
                      itemCount: positions.length,
                      itemBuilder: (BuildContext context, int index) {
                        return ListTile(
                          title: Text(positions[index]),
                        );
                      },
                    ),
                    ListView.builder(
                      shrinkWrap: true,
                      physics: const ClampingScrollPhysics(),
                      itemCount: orders.length,
                      itemBuilder: (BuildContext context, int index) {
                        return ListTile(
                          title: Text(orders[index]),
                        );
                      },
                    )
                  ],
                ),
              )
            ],
          ),
        ),
        floatingActionButtonLocation: FloatingActionButtonLocation.centerDocked,
        floatingActionButton: Padding(
          padding: const EdgeInsets.all(8.0),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: <Widget>[
              SizedBox(
                  width: tradeButtonWidth,
                  child: FloatingActionButton.extended(
                    heroTag: "btnBuy",
                    onPressed: () {
                      showBuySellSheet(context: context, direction: Direction.buy);
                    },
                    label: const Text("Buy"),
                    shape: tradeButtonShape,
                    backgroundColor: tradeTheme.buy,
                  )),
              const SizedBox(width: 20),
              SizedBox(
                  width: tradeButtonWidth,
                  child: FloatingActionButton.extended(
                    heroTag: "btnSell",
                    onPressed: () {
                      showBuySellSheet(context: context, direction: Direction.sell);
                    },
                    label: const Text("Sell"),
                    shape: tradeButtonShape,
                    backgroundColor: tradeTheme.sell,
                  )),
            ],
          ),
        ));
  }
}

showBuySellSheet({required BuildContext context, required Direction direction}) {
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
      return BuySellBottomSheet(direction: direction);
    },
  );
}
