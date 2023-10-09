import 'package:get_10101/common/domain/model.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart';
import 'package:get_10101/features/wallet/domain/lightning_invoice.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/logger/logger.dart';

class WalletService {
  const WalletService();

  Future<void> refreshWalletInfo() async {
    try {
      await rust.api.refreshWalletInfo();
    } catch (error) {
      logger.e("Failed to refresh wallet info: $error");
    }
  }

  /// Throws an exception if coordinator cannot provide required liquidity.
  Future<String?> createOnboardingInvoice(Amount amount, int liquidityOptionId) async {
    try {
      String invoice = await rust.api
          .createOnboardingInvoice(amountSats: amount.sats, liquidityOptionId: liquidityOptionId);
      logger.i("Successfully created invoice.");
      return invoice;
    } catch (error) {
      if (error is FfiException && error.message.contains("cannot provide required liquidity")) {
        rethrow;
      } else {
        logger.e("Error: $error", error: error);
        return null;
      }
    }
  }

  Future<String?> createInvoice(Amount? amount) async {
    try {
      String invoice = await rust.api.createInvoice(amountSats: amount?.sats);
      logger.i("Successfully created invoice.");
      return invoice;
    } catch (error) {
      logger.e("Error: $error", error: error);
    }
    return null;
  }

  Future<LightningInvoice?> decodeInvoice(String invoice) async {
    try {
      logger.d("Decoding invoice $invoice");
      rust.LightningInvoice lightningInvoice = await rust.api.decodeInvoice(invoice: invoice);
      logger.d("Successfully decoded invoice.");
      return LightningInvoice.fromApi(lightningInvoice);
    } catch (error) {
      logger.d("Failed to decode invoice: $error", error: error);
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
