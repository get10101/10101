import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;

class FundingRate {
  final double rate;
  final DateTime endDate;

  FundingRate({
    required this.rate,
    required this.endDate,
  });

  static FundingRate fromApi(bridge.FundingRate fundingRate) {
    return FundingRate(
        rate: fundingRate.rate,
        endDate: DateTime.fromMillisecondsSinceEpoch(fundingRate.endDate * 1000));
  }

  static bridge.FundingRate apiDummy() {
    return const bridge.FundingRate(rate: 0.0, endDate: 0);
  }
}
