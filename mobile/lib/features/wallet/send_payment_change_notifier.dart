import 'package:f_logs/model/flog/flog.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/features/wallet/application/wallet_service.dart';
import 'package:get_10101/features/wallet/domain/lightning_invoice.dart';

enum PendingPaymentState {
  pending,
  succeeded,
  failed,
}

class PendingPayment {
  PendingPaymentState state;
  final String rawInvoice;
  final LightningInvoice decodedInvoice;

  PendingPayment(
      {required this.rawInvoice,
      required this.decodedInvoice,
      this.state = PendingPaymentState.pending});
}

class SendPaymentChangeNotifier extends ChangeNotifier {
  final WalletService walletService;
  PendingPayment? _pendingPayment;

  SendPaymentChangeNotifier(this.walletService);

  sendPayment(String raw, LightningInvoice decoded) async {
    _pendingPayment = PendingPayment(rawInvoice: raw, decodedInvoice: decoded);

    // notify listeners about pending payment in state "pending"
    notifyListeners();

    try {
      await walletService.payInvoice(raw);
      _pendingPayment!.state = PendingPaymentState.succeeded;
    } catch (exception) {
      FLog.error(text: "Failed to submit order: $exception");
      _pendingPayment!.state = PendingPaymentState.failed;
    }

    // notify listeners about the status change of the pending order after submission
    notifyListeners();
  }

  PendingPayment? get pendingPayment => _pendingPayment;
}
