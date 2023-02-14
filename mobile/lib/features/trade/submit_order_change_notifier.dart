import 'package:flutter/material.dart';
import 'domain/trade_values.dart';

enum PendingOrderState {
  submitting,
  submittedSuccessfully,
  submissionFailed,
}

class PendingOrder {
  final TradeValues _tradeValues;
  PendingOrderState state = PendingOrderState.submitting;
  String? pendingOrderError;

  PendingOrder(this._tradeValues);
}

class SubmitOrderChangeNotifier extends ChangeNotifier {
  PendingOrder? _pendingOrder;

  submitPendingOrder(TradeValues tradeValues) async {
    _pendingOrder = PendingOrder(tradeValues);

    notifyListeners();

    // TODO: Actually submit the pending order...
    await Future.delayed(const Duration(seconds: 1));
    // TODO: Handle submit failure
    _pendingOrder!.state = PendingOrderState.submittedSuccessfully;

    notifyListeners();
  }

  PendingOrder? get pendingOrder => _pendingOrder;

  TradeValues? get pendingOrderValues => _pendingOrder?._tradeValues;
}
