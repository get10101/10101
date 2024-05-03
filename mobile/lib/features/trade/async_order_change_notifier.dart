import 'package:get_10101/logger/logger.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/features/trade/domain/order.dart';

class AsyncOrderChangeNotifier extends ChangeNotifier implements Subscriber {
  Order? asyncOrder;
  AsyncTrade? asyncTrade;

  // call this function to mark that the async trade has been processed.
  void removeAsyncTrade() {
    asyncTrade = null;
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
      this.asyncTrade = asyncTrade;
      notifyListeners();
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
