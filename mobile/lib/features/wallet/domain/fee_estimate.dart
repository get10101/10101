import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/common/domain/model.dart';

class FeeEstimation {
  final Amount perVbyte;
  final Amount total;

  FeeEstimation({required this.perVbyte, required this.total});

  static FeeEstimation fromAPI(rust.FeeEstimation fee) =>
      FeeEstimation(perVbyte: Amount(fee.satsPerVbyte), total: Amount(fee.totalSats));
}
