import 'package:flutter/material.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/submission_status_dialog.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/trade/candlestick_change_notifier.dart';
import 'package:get_10101/features/trade/contract_symbol_icon.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/order.dart';
import 'package:get_10101/features/trade/domain/position.dart';
import 'package:get_10101/features/trade/order_change_notifier.dart';
import 'package:get_10101/features/trade/order_list_item.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/trade/position_list_item.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/trade/trade_bottom_sheet.dart';
import 'package:candlesticks/candlesticks.dart';
import 'package:get_10101/features/trade/trade_bottom_sheet_confirmation.dart';
import 'package:get_10101/features/trade/trade_tabs.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';
import 'package:get_10101/util/constants.dart';

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

    SubmitOrderChangeNotifier submitOrderChangeNotifier =
        context.watch<SubmitOrderChangeNotifier>();

    if (submitOrderChangeNotifier.pendingOrder != null &&
        submitOrderChangeNotifier.pendingOrder!.state == PendingOrderState.submitting) {
      final pendingOrder = submitOrderChangeNotifier.pendingOrder;

      Amount pnl = Amount(0);
      if (pendingOrder!.close &&
          context.read<PositionChangeNotifier>().positions.containsKey(ContractSymbol.btcusd)) {
        final position = context.read<PositionChangeNotifier>().positions[ContractSymbol.btcusd];
        pnl = position!.unrealizedPnl != null ? position.unrealizedPnl! : Amount(0);
      }

      WidgetsBinding.instance.addPostFrameCallback((_) async {
        return await showDialog(
            context: context,
            useRootNavigator: true,
            builder: (BuildContext context) {
              return Selector<SubmitOrderChangeNotifier, PendingOrderState>(
                selector: (_, provider) => provider.pendingOrder!.state,
                builder: (context, state, child) {
                  const String title = "Submit Order";
                  Widget body = Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      SizedBox(
                        width: 200,
                        child: Wrap(
                          runSpacing: 10,
                          children: [
                            pendingOrder.close
                                ? ValueDataRow(
                                    type: ValueType.amount, value: pnl, label: "Unrealized P/L")
                                : ValueDataRow(
                                    type: ValueType.amount,
                                    value: submitOrderChangeNotifier.pendingOrderValues?.margin,
                                    label: "Margin"),
                            ValueDataRow(
                                type: ValueType.amount,
                                value: submitOrderChangeNotifier.pendingOrderValues?.fee,
                                label: "Fee")
                          ],
                        ),
                      ),
                      Padding(
                        padding: const EdgeInsets.only(top: 20, left: 10, right: 10, bottom: 5),
                        child: Text(
                            pendingOrder.close
                                ? "Your Position will be removed automatically once your ${submitOrderChangeNotifier.pendingOrderValues?.direction.name} order has been filled!"
                                : "Your Position will be shown automatically in the Positions tab once your ${submitOrderChangeNotifier.pendingOrderValues?.direction.name} order has been filled!",
                            style: DefaultTextStyle.of(context).style.apply(fontSizeFactor: 1.0)),
                      )
                    ],
                  );

                  switch (state) {
                    case PendingOrderState.submitting:
                      return SubmissionStatusDialog(
                          title: title, type: SubmissionStatusDialogType.pending, content: body);
                    case PendingOrderState.submittedSuccessfully:
                      return SubmissionStatusDialog(
                          title: title, type: SubmissionStatusDialogType.success, content: body);
                    case PendingOrderState.submissionFailed:
                      // TODO: This failure case has to be handled differently; are we planning to show orders that failed to submit in the order history?
                      return SubmissionStatusDialog(
                          title: title, type: SubmissionStatusDialogType.failure, content: body);
                  }
                },
              );
            });
      });
    }

    OrderChangeNotifier orderChangeNotifier = context.watch<OrderChangeNotifier>();
    PositionChangeNotifier positionChangeNotifier = context.watch<PositionChangeNotifier>();
    CandlestickChangeNotifier candlestickChangeNotifier =
        context.watch<CandlestickChangeNotifier>();
    TradeValuesChangeNotifier tradeValuesChangeNotifier = context.read<TradeValuesChangeNotifier>();

    SizedBox listBottomScrollSpace = const SizedBox(
      height: 60,
    );

    return Scaffold(
        body: Container(
          padding: const EdgeInsets.only(left: 15, right: 15),
          child: Column(
            children: [
              Row(
                children: [const ContractSymbolIcon(), Text(ContractSymbol.btcusd.label)],
              ),
              Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  SizedBox(
                    height: 250,
                    child: Candlesticks(
                      candles: candlestickChangeNotifier.candles,
                    ),
                  )
                ],
              ),
              Expanded(
                child: TradeTabs(
                  tabs: const [
                    "Positions",
                    "Orders",
                  ],
                  selectedIndex: positionChangeNotifier.positions.isEmpty ? 1 : 0,
                  keys: const [tradeScreenTabsPositions, tradeScreenTabsOrders],
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
                                      text: "Your order is being filled...\n\nCheck the ",
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
                                    text: "You currently don't have an open position...\n\n",
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
                            tradeValuesChangeNotifier.updateLeverage(
                                position.direction.opposite(), position.leverage);
                            tradeValuesChangeNotifier.updateQuantity(
                                position.direction.opposite(), position.quantity);
                            tradeBottomSheetConfirmation(
                                context: context,
                                direction: position.direction.opposite(),
                                onConfirmation: () {
                                  submitOrderChangeNotifier.closePosition(position);
                                  GoRouter.of(context).pop();
                                },
                                close: true);
                          },
                        );
                      },
                    ),
                    ListView.builder(
                      shrinkWrap: true,
                      physics: const ClampingScrollPhysics(),
                      itemCount: orderChangeNotifier.orders.isEmpty
                          ? 1
                          : orderChangeNotifier.orders.length + 1,
                      itemBuilder: (BuildContext context, int index) {
                        // If there are no positions we early-return with placeholder
                        if (orderChangeNotifier.orders.isEmpty) {
                          return RichText(
                              text: TextSpan(
                                  style: DefaultTextStyle.of(context).style,
                                  children: <TextSpan>[
                                const TextSpan(
                                    text: "You don't have any orders yet...\n\n",
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
                              ]));
                        }

                        // Spacer at the bottom of the list
                        if (index == orderChangeNotifier.orders.length) {
                          return listBottomScrollSpace;
                        }

                        return OrderListItem(
                            order: orderChangeNotifier.orders.values.toList()[index]);
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
                    key: tradeScreenButtonBuy,
                    heroTag: "btnBuy",
                    onPressed: () {
                      tradeBottomSheet(context: context, direction: Direction.long);
                    },
                    label: const Text("Buy"),
                    shape: tradeButtonShape,
                    backgroundColor: tradeTheme.buy,
                  )),
              const SizedBox(width: 20),
              SizedBox(
                  width: tradeButtonWidth,
                  child: FloatingActionButton.extended(
                    key: tradeScreenButtonSell,
                    heroTag: "btnSell",
                    onPressed: () {
                      tradeBottomSheet(context: context, direction: Direction.short);
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
