import 'dart:collection';
import 'dart:developer';

import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/features/trade/application/order_service.dart';
import 'package:get_10101/features/trade/domain/order.dart';

class OrderChangeNotifier extends ChangeNotifier implements Subscriber {
  late OrderService _orderService;
  HashMap<String, Order> orders = HashMap();

  Future<void> _create() async {
    List<Order> orders = await _orderService.fetchOrders();
    for (Order order in orders) {
      this.orders[order.id] = order;
    }

    notifyListeners();
  }

  OrderChangeNotifier.create(OrderService orderService) {
    _orderService = orderService;
    _create();
  }

  // TODO: This is not optimal, because we map the Order in the change notifier. We can do this, but it would be better to do this on the service level.
  @override
  void notify(bridge.Event event) {
    log("Receiving this in the order notifier: ${event.toString()}");

    if (event is bridge.Event_OrderUpdateNotification) {
      Order order = Order.fromApi(event.field0);
      orders[order.id] = order;
    } else {
      log("Received unexpected event: ${event.toString()}");
    }

    notifyListeners();
  }
}
