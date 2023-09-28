import '../../../common/domain/model.dart';

class ShareInvoice {
  final String rawInvoice;
  final Amount invoiceAmount;
  final bool isLightning;
  Amount? channelOpenFee;

  ShareInvoice(
      {required this.rawInvoice,
      required this.invoiceAmount,
      required this.isLightning,
      this.channelOpenFee});
}
