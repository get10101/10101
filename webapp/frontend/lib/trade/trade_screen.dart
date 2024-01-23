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
                Expanded(
                  child: Padding(
                    padding: const EdgeInsets.only(left: 8, right: 8),
                    child: Container(
                      decoration: BoxDecoration(
                        borderRadius: BorderRadius.circular(8),
                        color: Colors.grey[100],
                      ),
                      child: const Row(
                        children: [Expanded(child: OrderAndPositionTable())],
                      ),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ],
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
                      Expanded(
                        child: Container(
                          height: 120.0,
                          alignment: Alignment.center,
                          child: Column(
                            children: [
                              Expanded(
                                child: Padding(
                                  padding: const EdgeInsets.only(top: 8),
                                  child: Container(
                                    decoration: BoxDecoration(
                                      borderRadius: BorderRadius.circular(8),
                                      color: Colors.grey[100],
                                    ),
                                    child: const Row(
                                      children: [Expanded(child: OrderAndPositionTable())],
                                    ),
                                  ),
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
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
