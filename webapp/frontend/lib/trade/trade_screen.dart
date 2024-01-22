import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/trade/order_and_position_table.dart';
import 'package:get_10101/trade/trade_screen_order_form.dart';

class TradeScreen extends StatefulWidget {
  static const route = "/trade";

  const TradeScreen({super.key});

  @override
  State<TradeScreen> createState() => _TradeScreenState();
}

class _TradeScreenState extends State<TradeScreen> with SingleTickerProviderStateMixin {
  late final _tabController = TabController(length: 2, vsync: this);

  @override
  void initState() {
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    return ListView(
      children: [NewOrderWidget(tabController: _tabController), OrderAndPositionTable()],
    );
  }
}

class NewOrderWidget extends StatelessWidget {
  const NewOrderWidget({
    super.key,
    required TabController tabController,
  }) : _tabController = tabController;

  final TabController _tabController;

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisAlignment: MainAxisAlignment.start,
      crossAxisAlignment: CrossAxisAlignment.center,
      children: <Widget>[
        SizedBox(
          width: 300,
          child: TabBar(
            unselectedLabelColor: Colors.black,
            labelColor: tenTenOnePurple,
            controller: _tabController,
            tabs: const [
              Tab(
                text: 'Buy',
              ),
              Tab(
                text: 'Sell',
              ),
            ],
          ),
        ),
        SizedBox(
          height: 400,
          width: 300,
          child: TabBarView(
            controller: _tabController,
            children: <Widget>[
              NewOrderForm(isLong: true),
              NewOrderForm(
                isLong: false,
              )
            ],
          ),
        ),
      ],
    );
  }
}
