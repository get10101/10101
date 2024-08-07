import 'package:get_10101/features/trade/domain/funding_rate.dart';
import 'package:timeago/timeago.dart' as timeago;
import 'package:flutter/material.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/order.dart';
import 'package:get_10101/features/trade/domain/position.dart';
import 'package:get_10101/features/trade/funding_rate_change_notifier.dart';
import 'package:get_10101/features/trade/order_change_notifier.dart';
import 'package:get_10101/features/trade/order_list_item.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/trade/position_list_item.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/trade/trade_bottom_sheet.dart';
import 'package:get_10101/features/trade/trade_bottom_sheet_confirmation.dart';
import 'package:get_10101/features/trade/trade_change_notifier.dart';
import 'package:get_10101/features/trade/trade_list_item.dart';
import 'package:get_10101/features/trade/trade_tabs.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:get_10101/features/trade/tradingview_candlestick.dart';
import 'package:get_10101/util/constants.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

class TradeScreen extends StatelessWidget {
  static const route = "/trade";
  static const label = "Trade";

  const TradeScreen({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    TradeTheme tradeTheme = Theme.of(context).extension<TradeTheme>()!;

    const RoundedRectangleBorder tradeButtonShape = RoundedRectangleBorder(
      borderRadius: BorderRadius.all(
        Radius.circular(8),
      ),
    );

    const double tradeButtonWidth = 100.0;

    OrderChangeNotifier orderChangeNotifier = context.watch<OrderChangeNotifier>();
    PositionChangeNotifier positionChangeNotifier = context.watch<PositionChangeNotifier>();
    TradeChangeNotifier tradeChangeNotifier = context.watch<TradeChangeNotifier>();
    TradeValuesChangeNotifier tradeValuesChangeNotifier = context.read<TradeValuesChangeNotifier>();
    SubmitOrderChangeNotifier submitOrderChangeNotifier = context.read<SubmitOrderChangeNotifier>();

    SizedBox listBottomScrollSpace = const SizedBox(
      height: 60,
    );

    bool isBuyButtonEnabled = positionChangeNotifier.askPrice != null;
    bool isSellButtonEnabled = positionChangeNotifier.bidPrice != null;

    return Scaffold(
        body: Container(
          padding: const EdgeInsets.only(left: 15, right: 15),
          child: Column(
            children: [
              const SizedBox(height: 5),
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                children: [
                  Selector<TradeValuesChangeNotifier, double?>(selector: (_, provider) {
                    return provider.getAskPrice();
                  }, builder: (context, price, child) {
                    return LatestPriceWidget(
                      innerKey: tradeScreenAskPrice,
                      label: "Ask: ",
                      price: Usd.fromDouble(price ?? 0.0),
                    );
                  }),
                  Selector<TradeValuesChangeNotifier, double?>(selector: (_, provider) {
                    return provider.getBidPrice();
                  }, builder: (context, price, child) {
                    return LatestPriceWidget(
                      innerKey: tradeScreenBidPrice,
                      label: "Bid: ",
                      price: Usd.fromDouble(price ?? 0.0),
                    );
                  }),
                ],
              ),
              const SizedBox(height: 5),
              const Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  SizedBox(
                    height: 250,
                    child: TradingViewCandlestick(),
                  )
                ],
              ),
              const SizedBox(height: 5),
              Selector<FundingRateChangeNotifier, FundingRate?>(selector: (_, provider) {
                return provider.nextRate;
              }, builder: (context, rate, child) {
                return FundingRateWidget(rate: rate, innerKey: tradeScreenFundingRate);
              }),
              Expanded(
                child: TradeTabs(
                  tabs: const [
                    "Position",
                    "Orders",
                    "Trades",
                  ],
                  selectedIndex: 0,
                  keys: const [
                    tradeScreenTabsPositions,
                    tradeScreenTabsOrders,
                    tradeScreenTabsTrades
                  ],
                  tabBarViewChildren: [
                    ListView.builder(
                      shrinkWrap: true,
                      physics: const ClampingScrollPhysics(),
                      itemCount: positionChangeNotifier.positions.isEmpty
                          ? 1
                          : positionChangeNotifier.positions.length + 1,
                      itemBuilder: (BuildContext context, int index) {
                        // If there are no positions we early-return with placeholder
                        if (positionChangeNotifier.positions.isEmpty) {
                          // If we have an open order then let the user know
                          if (orderChangeNotifier.orders.values
                              .where((element) => element.state == OrderState.open)
                              .isNotEmpty) {
                            return RichText(
                                text: TextSpan(
                                    style: DefaultTextStyle.of(context).style,
                                    children: const <TextSpan>[
                                  TextSpan(
                                      text: "Your order is being filled\n\nCheck the ",
                                      style: TextStyle(color: Colors.grey)),
                                  TextSpan(text: "Orders", style: TextStyle(color: Colors.black)),
                                  TextSpan(
                                      text: " tab to see the status!",
                                      style: TextStyle(color: Colors.grey)),
                                ]));
                          }
                          return RichText(
                              text: TextSpan(
                                  style: DefaultTextStyle.of(context).style,
                                  children: <TextSpan>[
                                const TextSpan(
                                    text: "You don't have an open position.\n\n",
                                    style: TextStyle(color: Colors.grey)),
                                TextSpan(
                                    text: "Buy",
                                    style: TextStyle(
                                        color: tradeTheme.buy, fontWeight: FontWeight.bold)),
                                const TextSpan(text: " or ", style: TextStyle(color: Colors.grey)),
                                TextSpan(
                                    text: "Sell",
                                    style: TextStyle(
                                        color: tradeTheme.sell, fontWeight: FontWeight.bold)),
                                const TextSpan(
                                    text: " to open one!", style: TextStyle(color: Colors.grey)),
                              ]));
                        }

                        // Spacer at the bottom of the list
                        if (index == positionChangeNotifier.positions.length) {
                          return listBottomScrollSpace;
                        }

                        Position position = positionChangeNotifier.positions.values.toList()[index];

                        return PositionListItem(
                          position: position,
                          onClose: () async {
                            final direction = position.direction.opposite();

                            tradeValuesChangeNotifier.updateLeverage(direction, position.leverage);
                            tradeValuesChangeNotifier.updateQuantity(direction, position.quantity);

                            final tradeValues = tradeValuesChangeNotifier.fromDirection(direction);

                            tradeBottomSheetConfirmation(
                                context: context,
                                direction: direction,
                                channelOpeningParams: null,
                                onConfirmation: () {
                                  GoRouter.of(context).pop();

                                  submitOrderChangeNotifier.closePosition(
                                      position, tradeValues.price, tradeValues.fee);
                                },
                                tradeAction: TradeAction.closePosition);
                          },
                        );
                      },
                    ),
                    // If there are no orders we early-return with placeholder
                    orderChangeNotifier.orders.isEmpty
                        ? RichText(
                            text: TextSpan(
                                style: DefaultTextStyle.of(context).style,
                                children: <TextSpan>[
                                const TextSpan(
                                    text: "You don't have any orders yet.\n\n",
                                    style: TextStyle(color: Colors.grey)),
                                TextSpan(
                                    text: "Buy",
                                    style: TextStyle(
                                        color: tradeTheme.buy, fontWeight: FontWeight.bold)),
                                const TextSpan(text: " or ", style: TextStyle(color: Colors.grey)),
                                TextSpan(
                                    text: "Sell",
                                    style: TextStyle(
                                        color: tradeTheme.sell, fontWeight: FontWeight.bold)),
                                const TextSpan(
                                    text: " to create one!", style: TextStyle(color: Colors.grey)),
                              ]))
                        : SingleChildScrollView(
                            physics: const AlwaysScrollableScrollPhysics(),
                            child: Card(
                              child: Column(
                                  children: orderChangeNotifier.orders.values
                                      .map((e) => OrderListItem(order: e))
                                      .toList()),
                            )),
                    tradeChangeNotifier.trades.isEmpty
                        ? RichText(
                            text: TextSpan(
                                style: DefaultTextStyle.of(context).style,
                                children: <TextSpan>[
                                const TextSpan(
                                    text: "You don't have any trades yet.\n\n",
                                    style: TextStyle(color: Colors.grey)),
                                TextSpan(
                                    text: "Buy",
                                    style: TextStyle(
                                        color: tradeTheme.buy, fontWeight: FontWeight.bold)),
                                const TextSpan(text: " or ", style: TextStyle(color: Colors.grey)),
                                TextSpan(
                                    text: "Sell",
                                    style: TextStyle(
                                        color: tradeTheme.sell, fontWeight: FontWeight.bold)),
                                const TextSpan(
                                    text: " to create one!", style: TextStyle(color: Colors.grey)),
                              ]))
                        : SingleChildScrollView(
                            physics: const AlwaysScrollableScrollPhysics(),
                            child: Card(
                                child: Column(
                              children: tradeChangeNotifier.trades
                                  .map((trade) => TradeListItem(trade: trade))
                                  .toList(),
                            ))),
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
                  height: 50,
                  child: FloatingActionButton.extended(
                    key: tradeScreenButtonBuy,
                    heroTag: "btnBuy",
                    onPressed: () {
                      if (isBuyButtonEnabled) {
                        tradeBottomSheet(context: context, direction: Direction.long);
                      }
                    },
                    label: const Text(
                      "Buy",
                      style: TextStyle(color: Colors.white),
                    ),
                    shape: tradeButtonShape,
                    backgroundColor: isBuyButtonEnabled ? tradeTheme.buy : tradeTheme.disabled,
                  )),
              const SizedBox(width: 20),
              SizedBox(
                  width: tradeButtonWidth,
                  height: 50,
                  child: FloatingActionButton.extended(
                    key: tradeScreenButtonSell,
                    heroTag: "btnSell",
                    onPressed: () {
                      if (isSellButtonEnabled) {
                        tradeBottomSheet(context: context, direction: Direction.short);
                      }
                    },
                    label: const Text(
                      "Sell",
                      style: TextStyle(color: Colors.white),
                    ),
                    shape: tradeButtonShape,
                    backgroundColor: isBuyButtonEnabled ? tradeTheme.sell : tradeTheme.disabled,
                  )),
            ],
          ),
        ));
  }
}

class LatestPriceWidget extends StatelessWidget {
  final Usd price;
  final String label;
  final Key innerKey;

  const LatestPriceWidget(
      {super.key, required this.price, required this.label, required this.innerKey});

  @override
  Widget build(BuildContext context) {
    return RichText(
      key: innerKey,
      text: TextSpan(
        text: label,
        style: DefaultTextStyle.of(context).style,
        children: [
          TextSpan(
            text: "$price",
            style: const TextStyle(fontWeight: FontWeight.bold),
          ),
        ],
      ),
    );
  }
}

class FundingRateWidget extends StatelessWidget {
  final FundingRate? rate;
  final Key innerKey;

  const FundingRateWidget({super.key, required this.rate, required this.innerKey});

  @override
  Widget build(BuildContext context) {
    timeago.setLocaleMessages('en_custom', CustomEnMessages());

    return rate != null
        ? RichText(
            key: innerKey,
            text: TextSpan(
              text: rate!.rate.isNegative ? "Shorts pay longs" : "Longs pay shorts",
              style: DefaultTextStyle.of(context).style,
              children: [
                TextSpan(
                  text: " ${(rate!.rate.abs() * 100).toStringAsFixed(4)}% ",
                  style: const TextStyle(fontWeight: FontWeight.bold),
                ),
                TextSpan(
                  text: timeago.format(rate!.endDate, locale: 'en_custom', allowFromNow: true),
                )
              ],
            ),
          )
        : RichText(text: const TextSpan(text: "Next funding rate unavailable"));
  }
}

class CustomEnMessages extends timeago.EnMessages {
  @override
  String prefixFromNow() => 'in';

  @override
  String suffixFromNow() => '';
}
