import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/wallet/domain/confirmation_target.dart';
import 'package:get_10101/features/wallet/domain/destination.dart';
import 'package:get_10101/features/wallet/domain/fee.dart';
import 'package:get_10101/features/wallet/domain/fee_estimate.dart';
import 'package:get_10101/features/wallet/domain/share_payment_request.dart';
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

  Future<SharePaymentRequest?> createPaymentRequest(Amount? amount, String description) async {
    try {
      rust.PaymentRequest req =
          await rust.api.createPaymentRequest(amountSats: amount?.sats, description: description);
      logger.i("Successfully created payment request.");
      return SharePaymentRequest(bip21Uri: req.bip21, address: req.address, amount: amount);
    } catch (error) {
      logger.e("Error: $error", error: error);
    }
    return null;
  }

  Future<Destination?> decodeDestination(String destination) async {
    try {
      rust.Destination result = await rust.api.decodeDestination(destination: destination);

      if (result is rust.Destination_Bip21) {
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

  Future<String> sendOnChainPayment(Destination destination, Amount? amount, {Fee? fee}) {
    var feeApi = fee!.toAPI();
    var sats = amount!.sats;
    var address = destination.raw;
    logger.i("Sending payment of $amount to $address with fee $feeApi");

    return rust.api.sendPayment(address: address, amount: sats, fee: feeApi);
  }
}
