import 'package:flutter/material.dart';
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/domain/funding_channel_task.dart';
import 'package:get_10101/logger/logger.dart';

class FundingChannelChangeNotifier extends ChangeNotifier implements Subscriber {
  FundingChannelTaskStatus? status;
  String? error;

  @override
  void notify(bridge.Event event) async {
    if (event is bridge.Event_FundingChannelNotification) {
      logger.d("Received a funding channel task notification. ${event.field0}");
      var fromApi = FundingChannelTaskStatus.fromApi(event.field0);
      status = fromApi.$1;
      error = fromApi.$2;
      notifyListeners();
    }
  }
}
