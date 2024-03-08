import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/common/domain/model.dart';

class FeeEstimation {
  final double satsPerVbyte;
  final Amount total;

  FeeEstimation({required this.satsPerVbyte, required this.total});

  static FeeEstimation fromAPI(rust.FeeEstimation fee) =>
      FeeEstimation(satsPerVbyte: fee.satsPerVbyte, total: Amount(fee.totalSats));
}
