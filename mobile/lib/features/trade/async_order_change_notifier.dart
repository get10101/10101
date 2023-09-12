import 'package:f_logs/model/flog/flog.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/features/trade/application/order_service.dart';
import 'package:get_10101/features/trade/domain/order.dart';
import 'package:get_10101/features/trade/order_submission_status_dialog.dart';
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
  void notify(bridge.Event event) {
    if (event is bridge.Event_BackgroundNotification &&
        event.field0 is bridge.BackgroundTask_AsyncTrade) {
      AsyncTrade asyncTrade = AsyncTrade.fromApi(event.field0 as bridge.BackgroundTask_AsyncTrade);
      FLog.debug(text: "Received a async trade event. Reason: ${asyncTrade.orderReason}");
      showDialog(
        context: shellNavigatorKey.currentContext!,
        builder: (context) {
          Order? asyncOrder = context.watch<AsyncOrderChangeNotifier>().asyncOrder;

          OrderSubmissionStatusDialogType type = OrderSubmissionStatusDialogType.pendingSubmit;
          switch (asyncOrder?.state) {
            case OrderState.open:
              type = OrderSubmissionStatusDialogType.successfulSubmit;
            case OrderState.failed:
              type = OrderSubmissionStatusDialogType.failedFill;
            case OrderState.filled:
              type = OrderSubmissionStatusDialogType.filled;
            case null:
              type = OrderSubmissionStatusDialogType.pendingSubmit;
          }

          late Widget content;
          switch (asyncTrade.orderReason) {
            case OrderReason.expired:
              content = const Text("Your position has been closed due to expiry.");
            case OrderReason.manual:
              FLog.error(text: "A manual order should not appear as an async trade!");
              content = Container();
          }

          return OrderSubmissionStatusDialog(title: "Catching up!", type: type, content: content);
        },
      );
    } else if (event is bridge.Event_OrderUpdateNotification) {
      Order order = Order.fromApi(event.field0);
      if (order.reason != OrderReason.manual) {
        asyncOrder = order;
        notifyListeners();
      }
    } else {
      FLog.warning(text: "Received unexpected event: ${event.toString()}");
    }
  }
}
