import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/trade/order_history_table.dart';
import 'package:get_10101/trade/position_table.dart';

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
              text: 'Open Position',
            ),
            Tab(
              text: 'Order History',
            ),
          ],
        ),
        Expanded(
            child: TabBarView(
          controller: _tabController,
          children: const <Widget>[
            OpenPositionTable(),
            OrderHistoryTable(),
          ],
        ))
      ],
    );
  }
}
