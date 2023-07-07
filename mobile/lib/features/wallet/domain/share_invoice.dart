import '../../../common/domain/model.dart';

class ShareInvoice {
  final String rawInvoice;
  final Amount invoiceAmount;
  Amount? channelOpenFee;

  ShareInvoice({required this.rawInvoice, required this.invoiceAmount, this.channelOpenFee});
}
