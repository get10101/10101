import 'package:get_10101/logger/logger.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/common/task_status_dialog.dart';
import 'package:get_10101/features/trade/application/order_service.dart';
import 'package:get_10101/features/trade/domain/order.dart';
import 'package:provider/provider.dart';

class AsyncOrderChangeNotifier extends ChangeNotifier implements Subscriber {
  late OrderService _orderService;
  Order? asyncOrder;

  Future<void> initialize() async {
    Order? order = await _orderService.fetchAsyncOrder();

    if (order != null) {
      notifyListeners();
    }
  }

  AsyncOrderChangeNotifier(OrderService orderService) {
    _orderService = orderService;
  }

  @override
  void notify(bridge.Event event) async {
    if (event is bridge.Event_BackgroundNotification) {
      if (event.field0 is! bridge.BackgroundTask_AsyncTrade) {
        // ignoring other kinds of background tasks
        return;
      }
      AsyncTrade asyncTrade = AsyncTrade.fromApi(event.field0 as bridge.BackgroundTask_AsyncTrade);
      logger.d("Received a async trade event. Reason: ${asyncTrade.orderReason}");

      while (shellNavigatorKey.currentContext == null) {
        await Future.delayed(const Duration(milliseconds: 100)); // Adjust delay as needed
      }

      showDialog(
        context: shellNavigatorKey.currentContext!,
        builder: (context) {
          Order? asyncOrder = context.watch<AsyncOrderChangeNotifier>().asyncOrder;

          TaskStatus status = TaskStatus.pending;
          switch (asyncOrder?.state) {
            case OrderState.open:
            case OrderState.filling:
              status = TaskStatus.pending;
            case OrderState.failed:
            case OrderState.rejected:
              status = TaskStatus.failed;
            case OrderState.filled:
              status = TaskStatus.success;
            case null:
              status = TaskStatus.pending;
          }

          late Widget content;
          switch (asyncTrade.orderReason) {
            case OrderReason.expired:
              content = const Text("Your position has been closed due to expiry.");
            case OrderReason.liquidated:
              content = const Text("Your position has been closed due to liquidation.");
            case OrderReason.manual:
              logger.e("A manual order should not appear as an async trade!");
              content = Container();
          }

          return TaskStatusDialog(title: "Catching up!", status: status, content: content);
        },
      );
    } else if (event is bridge.Event_OrderUpdateNotification) {
      Order order = Order.fromApi(event.field0);
      if (order.reason != OrderReason.manual) {
        asyncOrder = order;
        notifyListeners();
      }
    } else {
      logger.w("Received unexpected event: ${event.toString()}");
    }
  }
}
