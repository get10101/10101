import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';

enum PaymentStatus {
  pending,
  success,
  failed,
}

class PaymentChangeNotifier extends ChangeNotifier implements Subscriber {
  PaymentStatus _status = PaymentStatus.pending;

  void waitForPayment() {
    _status = PaymentStatus.pending;
  }

  void failPayment() {
    _status = PaymentStatus.failed;
    super.notifyListeners();
  }

  PaymentStatus getPaymentStatus() => _status;

  @override
  void notify(Event event) {
    if (event is bridge.Event_PaymentSent) {
      _status = PaymentStatus.success;
      super.notifyListeners();
    }

    if (event is bridge.Event_PaymentFailed) {
      _status = PaymentStatus.failed;
      super.notifyListeners();
    }
  }
}
