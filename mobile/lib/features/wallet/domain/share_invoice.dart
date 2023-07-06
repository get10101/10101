import '../../../common/domain/model.dart';

class ShareInvoice {
  final String rawInvoice;
  final Amount invoiceAmount;

  ShareInvoice({required this.rawInvoice, required this.invoiceAmount});
}
