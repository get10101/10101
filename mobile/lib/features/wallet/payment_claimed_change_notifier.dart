import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/logger/logger.dart';

class PaymentClaimedChangeNotifier extends ChangeNotifier implements Subscriber {
  bool _claimed = false;

  void waitForPayment() => _claimed = false;

  bool isClaimed() => _claimed;

  @override
  void notify(bridge.Event event) {
    if (event is bridge.Event_PaymentClaimed) {
      final paymentAmountMsats = event.field0;
      final paymentHash = event.field1;
      final paymentAmountSats = paymentAmountMsats / 1000;

      logger.i("Amount : $paymentAmountSats hash: $paymentHash");
      _claimed = true;
      super.notifyListeners();
    }
  }
}
