import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/direction.dart';
import 'package:get_10101/trade/order_change_notifier.dart';
import 'package:get_10101/services/order_service.dart';
import 'package:intl/intl.dart';
import 'package:provider/provider.dart';

class OrderHistoryTable extends StatelessWidget {
  const OrderHistoryTable({super.key});

  @override
  Widget build(BuildContext context) {
    final orderChangeNotified = context.watch<OrderChangeNotifier>();
    final orders = orderChangeNotified.getOrders();

    if (orders == null) {
      return const Center(child: CircularProgressIndicator());
    }

    if (orders.isEmpty) {
      return const Center(child: Text('No data available'));
    } else {
      return buildTable(orders, context);
    }
  }

  Widget buildTable(List<Order> orders, BuildContext context) {
    orders.sort((a, b) => b.creationTimestamp.compareTo(a.creationTimestamp));

    return Table(
      border: const TableBorder(verticalInside: BorderSide(width: 0.5, color: Colors.black)),
      defaultVerticalAlignment: TableCellVerticalAlignment.middle,
      columnWidths: const {
        0: MinColumnWidth(FixedColumnWidth(100.0), FractionColumnWidth(0.1)),
        1: MinColumnWidth(FixedColumnWidth(100.0), FractionColumnWidth(0.1)),
        2: MinColumnWidth(FixedColumnWidth(100.0), FractionColumnWidth(0.1)),
        3: MinColumnWidth(FixedColumnWidth(100.0), FractionColumnWidth(0.1)),
        4: MinColumnWidth(FixedColumnWidth(200.0), FractionColumnWidth(0.2)),
      },
      children: [
        TableRow(
          decoration: BoxDecoration(
            color: tenTenOnePurple.shade300,
            border: const Border(bottom: BorderSide(width: 0.5, color: Colors.black)),
          ),
          children: [
            buildHeaderCell('State'),
            buildHeaderCell('Price'),
            buildHeaderCell('Quantity'),
            buildHeaderCell('Leverage'),
            buildHeaderCell('Timestamp'),
          ],
        ),
        for (var order in orders)
          TableRow(
            children: [
              buildTableCell(Tooltip(message: order.state.asString, child: stateToIcon(order))),
              // buildTableCell(Text(order.id)),
              buildTableCell(Text(order.price != null ? order.price.toString() : "NaN")),
              buildTableCell(Text(order.direction == Direction.short
                  ? "-${order.quantity}"
                  : "+${order.quantity}")),
              buildTableCell(Text("${order.leverage.formatted()}x")),
              buildTableCell(
                  Text("${DateFormat('dd-MM-yyyy – HH:mm').format(order.creationTimestamp)} UTC")),
            ],
          ),
      ],
    );
  }

  Widget stateToIcon(Order order) {
    const double size = 16.0;
    var icon = switch (order.state) {
      OrderState.initial =>
        const SizedBox(width: size, height: size, child: CircularProgressIndicator()),
      OrderState.rejected => const Icon(
          FontAwesomeIcons.circleExclamation,
          size: size,
        ),
      OrderState.open =>
        const SizedBox(width: size, height: size, child: CircularProgressIndicator()),
      OrderState.filling =>
        const SizedBox(width: size, height: size, child: CircularProgressIndicator()),
      OrderState.failed => const Icon(
          FontAwesomeIcons.circleExclamation,
          color: Colors.red,
          size: size,
        ),
      OrderState.filled => const Icon(
          FontAwesomeIcons.check,
          size: size,
        ),
      OrderState.unknown => const Icon(
          FontAwesomeIcons.circleExclamation,
          size: size,
        )
    };
    return icon;
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
          child: SelectionArea(
        child: Center(
            child: Container(
                padding: const EdgeInsets.all(10), alignment: Alignment.center, child: child)),
      ));
}
