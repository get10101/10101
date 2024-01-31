import 'package:flutter/material.dart';
import 'package:flutter/widgets.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/direction.dart';
import 'package:get_10101/common/model.dart';
import 'package:get_10101/trade/position_change_notifier.dart';
import 'package:get_10101/trade/position_service.dart';
import 'package:get_10101/trade/quote_change_notifier.dart';
import 'package:get_10101/trade/quote_service.dart';
import 'package:get_10101/trade/trade_confirmation_dialog.dart';
import 'package:intl/intl.dart';
import 'package:provider/provider.dart';

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
      border: const TableBorder(verticalInside: BorderSide(width: 0.5, color: Colors.black)),
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
            border: const Border(bottom: BorderSide(width: 0.5, color: Colors.black)),
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
              buildTableCell(Text(position.direction == "Short"
                  ? "-${position.quantity}"
                  : "+${position.quantity}")),
              buildTableCell(Text(position.averageEntryPrice.toString())),
              buildTableCell(Text(position.liquidationPrice.toString())),
              buildTableCell(Text(position.collateral.toString())),
              buildTableCell(Text(position.leverage.formatted())),
              buildTableCell(Text(position.pnlSats.toString())),
              buildTableCell(
                  Text("${DateFormat('dd-MM-yyyy – HH:mm').format(position.expiry)} UTC")),
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
                                    ? Amount(position.collateral.sats +
                                        (position.pnlSats?.sats ?? 0) -
                                        (position.closingFee?.sats ?? 0))
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
                style: const TextStyle(fontWeight: FontWeight.normal, color: Colors.white))));
  }

  TableCell buildTableCell(Widget child) => TableCell(
      child: Center(
          child: Container(
              padding: const EdgeInsets.all(10), alignment: Alignment.center, child: child)));
}
