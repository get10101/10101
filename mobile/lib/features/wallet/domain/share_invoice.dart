import '../../../common/domain/model.dart';

class ShareInvoice {
  final String rawInvoice;
  final Amount invoiceAmount;
  final bool isLightning;

  ShareInvoice({required this.rawInvoice, required this.invoiceAmount, required this.isLightning});
}
