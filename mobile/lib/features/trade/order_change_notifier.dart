import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/application/order_service.dart';
import 'package:get_10101/features/trade/domain/order.dart';

class OrderChangeNotifier extends ChangeNotifier {
  final OrderService orderService;
  List<Order> orders = List.empty();

  OrderChangeNotifier(this.orderService) {
    updateOrders();
  }

  updateOrders() async {
    orders = await orderService.fetchOrders();
    notifyListeners();
  }
}
