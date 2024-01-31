import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/trade/order_history_table.dart';
import 'package:get_10101/trade/position_table.dart';
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
    return LayoutBuilder(builder: (context, constraints) {
      if (constraints.maxWidth > 600) {
        return _buildHorizontalWidget(constraints);
      } else {
        return _buildHVerticalWidget(constraints, constraints);
      }
    });
  }

  Widget _buildHorizontalWidget(BoxConstraints constraints) {
    return Padding(
      padding: const EdgeInsets.all(8.0),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(
            flex: 1,
            child: Column(
              children: [
                ConstrainedBox(
                  constraints: BoxConstraints(
                    maxHeight: constraints.maxHeight - 16,
                  ),
                  child: Container(
                    height: double.infinity,
                    decoration: BoxDecoration(
                      borderRadius: BorderRadius.circular(8),
                      color: Colors.grey[100],
                    ),
                    child: Row(
                      children: [
                        Expanded(
                            child: Center(child: NewOrderWidget(tabController: _tabController))),
                      ],
                    ),
                  ),
                ),
              ],
            ),
          ),
          Expanded(
            flex: 2,
            child: Column(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Visibility(
                  visible: false,
                  child: Expanded(
                    child: Padding(
                      padding: const EdgeInsets.only(left: 8, right: 8, bottom: 8),
                      child: Container(
                        height: double.infinity,
                        decoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(8),
                          color: Colors.grey[100],
                        ),
                        child: const Row(
                          children: [Expanded(child: Center(child: Text("Chart")))],
                        ),
                      ),
                    ),
                  ),
                ),
                createTableWidget(const OpenPositionTable(), "Open Positions"),
                const SizedBox(
                  height: 10,
                ),
                createTableWidget(const OrderHistoryTable(), "Order History"),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Expanded createTableWidget(Widget child, String title) {
    return Expanded(
      child: Padding(
        padding: const EdgeInsets.only(left: 8, right: 8),
        child: Container(
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(8),
            color: Colors.grey[100],
          ),
          child: Column(
            children: [
              Row(
                children: [
                  Expanded(
                    child: Container(
                        decoration: BoxDecoration(
                          color: tenTenOnePurple.shade300,
                          border: Border.all(
                            width: 0.5,
                          ),
                          borderRadius: const BorderRadius.only(
                              topLeft: Radius.circular(10), topRight: Radius.circular(10)),
                        ),
                        padding: const EdgeInsets.all(10),
                        alignment: Alignment.center,
                        child: Text(title,
                            textAlign: TextAlign.center,
                            style:
                                const TextStyle(fontWeight: FontWeight.bold, color: Colors.white))),
                  ),
                ],
              ),
              Row(
                children: [
                  Expanded(child: child),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildHVerticalWidget(BoxConstraints constraints, BoxConstraints viewportConstraints) {
    return Padding(
        padding: const EdgeInsets.all(8.0),
        child: SingleChildScrollView(
            child: ConstrainedBox(
                constraints: BoxConstraints(
                  minHeight: viewportConstraints.maxHeight,
                ),
                child: IntrinsicHeight(
                  child: Column(
                    children: <Widget>[
                      Container(
                        height: 480.0,
                        alignment: Alignment.center,
                        decoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(8),
                          color: Colors.grey[100],
                        ),
                        child: NewOrderWidget(tabController: _tabController),
                      ),
                      const SizedBox(
                        height: 10,
                      ),
                      createTableWidget(const OpenPositionTable(), "Open Positions"),
                      const SizedBox(
                        height: 10,
                      ),
                      createTableWidget(const OrderHistoryTable(), "Order History"),
                    ],
                  ),
                ))));
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
            children: const <Widget>[NewOrderForm(isLong: true), NewOrderForm(isLong: false)],
          ),
        ),
      ],
    );
  }
}
