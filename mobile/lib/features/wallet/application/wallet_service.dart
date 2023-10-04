import 'package:f_logs/f_logs.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/wallet/domain/lightning_invoice.dart';
import 'package:get_10101/ffi.dart' as rust;

class WalletService {
  const WalletService();

  Future<void> refreshWalletInfo() async {
    try {
      await rust.api.refreshWalletInfo();
    } catch (error) {
      FLog.error(text: "Failed to refresh wallet info: $error");
    }
  }

  /// Throws an exception if coordinator cannot provide required liquidity.
  Future<String?> createOnboardingInvoice(Amount amount, int liquidityOptionId) async {
    try {
      String invoice = await rust.api
          .createOnboardingInvoice(amountSats: amount.sats, liquidityOptionId: liquidityOptionId);
      FLog.info(text: "Successfully created invoice.");
      return invoice;
    } catch (error) {
      if (error is FfiException && error.message.contains("cannot provide required liquidity")) {
        rethrow;
      } else {
        FLog.error(text: "Error: $error", exception: error);
        return null;
      }
    }
  }

  Future<String?> createInvoice(Amount? amount) async {
    try {
      String invoice = await rust.api.createInvoice(amountSats: amount?.sats);
      FLog.info(text: "Successfully created invoice.");
      return invoice;
    } catch (error) {
      FLog.error(text: "Error: $error", exception: error);
    }
    return null;
  }

  Future<LightningInvoice?> decodeInvoice(String invoice) async {
    try {
      FLog.debug(text: "Decoding invoice $invoice");
      rust.LightningInvoice lightningInvoice = await rust.api.decodeInvoice(invoice: invoice);
      FLog.debug(text: "Successfully decoded invoice.");
      return LightningInvoice.fromApi(lightningInvoice);
    } catch (error) {
      FLog.debug(text: "Failed to decode invoice: $error", exception: error);
      return null;
    }
  }

  Future<void> payInvoice(String invoice) async {
    await rust.api.sendPayment(invoice: invoice);
  }

  String getUnusedAddress() {
    return rust.api.getUnusedAddress();
  }
}
