import 'package:flutter/material.dart';
import 'package:f_logs/model/flog/flog.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/features/trade/application/order_service.dart';
import 'package:get_10101/features/trade/domain/order.dart';

class OrderChangeNotifier extends ChangeNotifier implements Subscriber {
  late OrderService _orderService;
  Map<String, Order> orders = {};

  Future<void> initialize() async {
    List<Order> orders = await _orderService.fetchOrders();
    for (Order order in orders) {
      this.orders[order.id] = order;
    }

    sortOrderByTimestampDesc();
    notifyListeners();
  }

  OrderChangeNotifier(OrderService orderService) {
    _orderService = orderService;
  }

  // TODO: This is not optimal, because we map the Order in the change notifier. We can do this, but it would be better to do this on the service level.
  @override
  void notify(bridge.Event event) {
    if (event is bridge.Event_OrderUpdateNotification) {
      Order order = Order.fromApi(event.field0);
      orders[order.id] = order;

      sortOrderByTimestampDesc();

      notifyListeners();
    } else {
      FLog.warning(text: "Received unexpected event: ${event.toString()}");
    }
  }

  void sortOrderByTimestampDesc() {
    orders = Map<String, Order>.fromEntries(orders.entries.toList()
      ..sort((e1, e2) => e2.value.creationTimestamp.compareTo(e1.value.creationTimestamp)));
  }
}
