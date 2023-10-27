import 'package:get_10101/common/domain/model.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart';
import 'package:get_10101/features/wallet/domain/destination.dart';
import 'package:get_10101/features/wallet/domain/share_payment_request.dart';
import 'package:get_10101/features/wallet/domain/wallet_type.dart';
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
  Future<String?> createOnboardingInvoice(
      Amount amount, int liquidityOptionId, Amount feeSats) async {
    try {
      String invoice = await rust.api.createOnboardingInvoice(
          amountSats: amount.sats, liquidityOptionId: liquidityOptionId, feeSats: feeSats.sats);
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

  Future<SharePaymentRequest?> createPaymentRequest(Amount? amount) async {
    try {
      rust.PaymentRequest req = await rust.api.createPaymentRequest(amountSats: amount?.sats);
      logger.i("Successfully created payment request.");
      return SharePaymentRequest(
          lightningInvoice: req.lightning, bip21Uri: req.bip21, amount: amount);
    } catch (error) {
      logger.e("Error: $error", error: error);
    }
    return null;
  }

  Future<Destination?> decodeDestination(String destination) async {
    try {
      rust.Destination result = await rust.api.decodeDestination(destination: destination);

      if (result is rust.Destination_Bolt11) {
        return LightningInvoice.fromApi(result, destination);
      } else if (result is rust.Destination_Bip21) {
        return OnChainAddress.fromApi(result);
      } else if (result is rust.Destination_OnChainAddress) {
        return OnChainAddress.fromAddress(result);
      } else {
        return null;
      }
    } catch (error) {
      logger.d("Failed to decode invoice: $error", error: error);
      return null;
    }
  }

  Future<void> sendPayment(Destination destination, Amount? amount) async {
    logger.i("Sending payment of $amount");

    rust.SendPayment payment;
    switch (destination.getWalletType()) {
      case WalletType.lightning:
        payment = rust.SendPayment_Lightning(invoice: destination.raw, amount: amount?.sats);
      case WalletType.onChain:
        payment = rust.SendPayment_OnChain(address: destination.raw, amount: amount!.sats);
      default:
        throw Exception("unsupported wallet type: ${destination.getWalletType().name}");
    }
    await rust.api.sendPayment(payment: payment);
  }

  String getUnusedAddress() {
    return rust.api.getUnusedAddress();
  }
}
