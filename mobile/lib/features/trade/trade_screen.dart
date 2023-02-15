import 'package:flutter/material.dart';
import 'package:get_10101/common/submission_status_dialog.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/trade/contract_symbol_icon.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/trade/trade_bottom_sheet.dart';
import 'package:get_10101/features/trade/candlestick_chart.dart';
import 'package:get_10101/features/trade/trade_tabs.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:provider/provider.dart';

class TradeScreen extends StatelessWidget {
  static const route = "/trade";
  static const label = "Trade";

  const TradeScreen({Key? key}) : super(key: key);

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

    SubmitOrderChangeNotifier submitOrderChangeNotifier =
        context.watch<SubmitOrderChangeNotifier>();

    if (submitOrderChangeNotifier.pendingOrder != null &&
        submitOrderChangeNotifier.pendingOrder!.state == PendingOrderState.submitting) {
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
                            ValueDataRow(
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
                            "Your Position will be shown automatically in the Orders tab once your ${submitOrderChangeNotifier.pendingOrderValues?.direction.name} order has been filled!",
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
