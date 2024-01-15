import 'package:get_10101/common/domain/model.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart';
import 'package:get_10101/features/wallet/domain/confirmation_target.dart';
import 'package:get_10101/features/wallet/domain/destination.dart';
import 'package:get_10101/features/wallet/domain/fee.dart';
import 'package:get_10101/features/wallet/domain/fee_estimate.dart';
import 'package:get_10101/features/wallet/domain/share_payment_request.dart';
import 'package:get_10101/features/wallet/domain/wallet_type.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/logger/logger.dart';

class WalletService {
  const WalletService();

  Future<void> refreshLightningWallet() async {
    try {
      await rust.api.refreshLightningWallet();
    } catch (error) {
      logger.e("Failed to refresh lightning wallet: $error");
    }
  }

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

  Future<SharePaymentRequest?> createPaymentRequest(
      Amount? amount, bool usdpInvoice, String description) async {
    try {
      rust.PaymentRequest req = await rust.api.createPaymentRequest(
          amountSats: amount?.sats, isUsdp: usdpInvoice, description: description);
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

  Future<Map<ConfirmationTarget, FeeEstimation>> calculateFeesForOnChain(
      String address, Amount amount) async {
    final Map<ConfirmationTarget, FeeEstimation> map = {};

    final fees = await rust.api.calculateAllFeesForOnChain(address: address, amount: amount.sats);
    for (int i = 0; i < ConfirmationTarget.values.length; i++) {
      map[ConfirmationTarget.values[i]] = FeeEstimation.fromAPI(fees[i]);
    }

    return map;
  }

  Future<int> estimateFeeMsat(Destination destination, Amount? amount, Fee? fee) async {
    return switch (fee) {
      null ||
      PriorityFee() =>
        await rust.api.sendPreflightProbe(payment: _createPayment(destination, amount, fee: fee)),
      CustomFeeRate() => fee.amount.sats * 1000,
    };
  }

  Future<void> sendPayment(Destination destination, Amount? amount, {Fee? fee}) async {
    logger.i("Sending payment of $amount");
    await rust.api.sendPayment(payment: _createPayment(destination, amount, fee: fee));
  }

  String getUnusedAddress() {
    return rust.api.getUnusedAddress();
  }
}

rust.SendPayment _createPayment(Destination destination, Amount? amount, {Fee? fee}) {
  switch (destination.getWalletType()) {
    case WalletType.lightning:
      return rust.SendPayment_Lightning(invoice: destination.raw, amount: amount?.sats);
    case WalletType.onChain:
      return rust.SendPayment_OnChain(
          address: destination.raw, amount: amount!.sats, fee: fee!.toAPI());
    default:
      throw Exception("unsupported wallet type: ${destination.getWalletType().name}");
  }
}
