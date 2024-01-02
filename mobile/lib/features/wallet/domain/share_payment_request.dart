import 'package:get_10101/common/domain/model.dart';

class SharePaymentRequest {
  // TODO(bonomat): this should be removed
  final String lightningInvoice;
  final String bip21Uri;
  final Amount? amount;

  SharePaymentRequest(
      {required this.lightningInvoice, required this.bip21Uri, required this.amount});
}
