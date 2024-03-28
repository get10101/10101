import 'dart:async';

import 'package:flutter/material.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/services/order_service.dart';

class OrderChangeNotifier extends ChangeNotifier {
  final OrderService service;
  late Timer timer;

  List<Order>? _orders;

  OrderChangeNotifier(this.service) {
    _refresh();
    Timer.periodic(const Duration(seconds: 2), (timer) async {
      _refresh();
    });
  }

  void _refresh() async {
    try {
      final orders = await service.fetchOrders();
      _orders = orders;

      super.notifyListeners();
    } catch (error) {
      logger.e(error);
    }
  }

  List<Order>? getOrders() => _orders;

  @override
  void dispose() {
    super.dispose();
    timer.cancel();
  }
}
