import 'package:f_logs/model/flog/flog.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/application/order_service.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
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
  final OrderService orderService;
  PendingOrder? _pendingOrder;

  SubmitOrderChangeNotifier(this.orderService);

  submitPendingOrder(TradeValues tradeValues) async {
    _pendingOrder = PendingOrder(tradeValues);

    // notify listeners about pending order in state "pending"
    notifyListeners();

    try {
      await orderService.submitMarketOrder(
          tradeValues.leverage, tradeValues.quantity, ContractSymbol.btcusd, tradeValues.direction);
      _pendingOrder!.state = PendingOrderState.submittedSuccessfully;
    } catch (exception) {
      FLog.error(text: "Failed to submit order: $exception");
      _pendingOrder!.state = PendingOrderState.submissionFailed;
    }

    // notify listeners about the status change of the pending order after submission
    notifyListeners();
  }

  PendingOrder? get pendingOrder => _pendingOrder;

  TradeValues? get pendingOrderValues => _pendingOrder?._tradeValues;
}
