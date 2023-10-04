import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';

class PaymentClaimedChangeNotifier extends ChangeNotifier implements Subscriber {
  bool _claimed = false;

  void waitForPayment() {
    _claimed = false;
  }

  bool isClaimed() {
    return _claimed;
  }

  @override
  void notify(Event event) {
    if (event is bridge.Event_PaymentClaimed) {
      _claimed = true;
      super.notifyListeners();
    }
  }
}
