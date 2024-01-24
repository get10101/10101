import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/contract_symbol_icon.dart';
import 'package:get_10101/common/direction.dart';
import 'package:get_10101/common/model.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/common/theme.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/trade/new_order_service.dart';
import 'package:get_10101/trade/position_change_notifier.dart';
import 'package:get_10101/trade/position_service.dart';
import 'package:get_10101/trade/quote_change_notifier.dart';
import 'package:get_10101/trade/quote_service.dart';
import 'package:intl/intl.dart';
import 'package:provider/provider.dart';

class OrderAndPositionTable extends StatefulWidget {
  const OrderAndPositionTable({super.key});

  @override
  OrderAndPositionTableState createState() => OrderAndPositionTableState();
}

class OrderAndPositionTableState extends State<OrderAndPositionTable>
    with SingleTickerProviderStateMixin {
  late final _tabController = TabController(length: 2, vsync: this);

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisAlignment: MainAxisAlignment.start,
      crossAxisAlignment: CrossAxisAlignment.center,
      children: <Widget>[
        TabBar(
          unselectedLabelColor: Colors.black,
          labelColor: tenTenOnePurple,
          controller: _tabController,
          isScrollable: false,
          tabs: const [
            Tab(
              text: 'Open',
            ),
            Tab(
              text: 'Pending',
            ),
          ],
        ),
        Expanded(
            child: TabBarView(
          controller: _tabController,
          children: const <Widget>[
            OpenPositionTable(),
            Text("Pending"),
          ],
        ))
      ],
    );
  }
}

class OpenPositionTable extends StatelessWidget {
  const OpenPositionTable({super.key});

  @override
  Widget build(BuildContext context) {
    final positionChangeNotifier = context.watch<PositionChangeNotifier>();
    final positions = positionChangeNotifier.getPositions();
    final quoteChangeNotifier = context.watch<QuoteChangeNotifier>();
    final quote = quoteChangeNotifier.getBestQuote();

    if (positions == null) {
      return const Center(child: CircularProgressIndicator());
    }

    if (positions.isEmpty) {
      return const Center(child: Text('No data available'));
    } else {
      return buildTable(positions, quote, context);
    }
  }

  Widget buildTable(List<Position> positions, BestQuote? bestQuote, BuildContext context) {
    return Table(
      border: TableBorder.symmetric(inside: const BorderSide(width: 2, color: Colors.black)),
      defaultVerticalAlignment: TableCellVerticalAlignment.middle,
      columnWidths: const {
        0: MinColumnWidth(FixedColumnWidth(100.0), FractionColumnWidth(0.1)),
        1: MinColumnWidth(FixedColumnWidth(100.0), FractionColumnWidth(0.1)),
        2: MinColumnWidth(FixedColumnWidth(100.0), FractionColumnWidth(0.1)),
        3: MinColumnWidth(FixedColumnWidth(150.0), FractionColumnWidth(0.1)),
        4: MinColumnWidth(FixedColumnWidth(100.0), FractionColumnWidth(0.1)),
        5: MinColumnWidth(FixedColumnWidth(100.0), FractionColumnWidth(0.1)),
        6: MinColumnWidth(FixedColumnWidth(200.0), FractionColumnWidth(0.25)),
        7: FixedColumnWidth(100),
      },
      children: [
        TableRow(
          decoration: BoxDecoration(
            color: tenTenOnePurple.shade300,
            border: Border.all(
              width: 1,
            ),
            borderRadius: const BorderRadius.only(
                topLeft: Radius.circular(10), topRight: Radius.circular(10)),
          ),
          children: [
            buildHeaderCell('Quantity'),
            buildHeaderCell('Entry Price'),
            buildHeaderCell('Liquidation Price'),
            buildHeaderCell('Margin'),
            buildHeaderCell('Leverage'),
            buildHeaderCell('Unrealized PnL'),
            buildHeaderCell('Expiry'),
            buildHeaderCell('Action'),
          ],
        ),
        for (var position in positions)
          TableRow(
            children: [
              buildTableCell(Text(position.quantity.toString())),
              buildTableCell(Text(position.averageEntryPrice.toString())),
              buildTableCell(Text(position.liquidationPrice.toString())),
              buildTableCell(Text(position.collateral.toString())),
              buildTableCell(Text(position.leverage.formatted())),
              buildTableCell(Text(position.pnlSats.toString())),
              buildTableCell(
                  Text("${DateFormat('dd-MM-yyyy – HH:mm').format(position.expiry)} CET")),
              buildTableCell(Center(
                child: SizedBox(
                  width: 100,
                  child: ElevatedButton(
                      onPressed: () {
                        showDialog(
                            context: context,
                            builder: (BuildContext context) {
                              return TradeConfirmationDialog(
                                direction: Direction.fromString(position.direction),
                                onConfirmation: () {},
                                bestQuote: bestQuote,
                                pnl: position.pnlSats,
                                fee: position.closingFee,
                                payout: position.closingFee != null
                                    ? Amount(position.collateral.sats + position.closingFee!.sats)
                                    : null,
                                leverage: position.leverage,
                                quantity: position.quantity,
                              );
                            });
                      },
                      child: const Text("Close", style: TextStyle(fontSize: 16))),
                ),
              )),
            ],
          ),
      ],
    );
  }

  TableCell buildHeaderCell(String text) {
    return TableCell(
        child: Container(
            padding: const EdgeInsets.all(10),
            alignment: Alignment.center,
            child: Text(text,
                textAlign: TextAlign.center,
                style: const TextStyle(fontWeight: FontWeight.bold, color: Colors.white))));
  }

  TableCell buildTableCell(Widget child) => TableCell(
      child: Center(
          child: Container(
              padding: const EdgeInsets.all(10), alignment: Alignment.center, child: child)));
}

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
                              onPressed: () {
                                NewOrderService.postNewOrder(
                                        leverage, quantity, direction == Direction.long.opposite())
                                    .then((orderId) {
                                  showSnackBar(
                                      messenger, "Closing order created. Order id: $orderId.");
                                  Navigator.pop(context);
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
