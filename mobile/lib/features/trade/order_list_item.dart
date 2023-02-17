import 'package:flutter/material.dart';

import 'contract_symbol_icon.dart';
import 'domain/order.dart';

class OrderListItem extends StatelessWidget {
  const OrderListItem({super.key, required this.order});

  final Order order;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: ListTile(
        leading: const ContractSymbolIcon(),
        title: Text(
            "${order.direction.nameU} Order for ${order.quantity} contracts x${order.leverage.leverage}"),
        subtitle: Text(order.status.name),
      ),
    );
  }
}
