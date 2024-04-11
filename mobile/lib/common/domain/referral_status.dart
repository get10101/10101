import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;

class ReferralStatus {
  final String referralCode;
  final int numberOfActivatedReferrals;
  final int numberOfTotalReferrals;
  final int referralTier;
  final double referralFeeBonus;

  /// The type of this referral status
  final BonusStatusType bonusStatusType;

  const ReferralStatus(
      {required this.referralCode,
      required this.numberOfActivatedReferrals,
      required this.numberOfTotalReferrals,
      required this.referralTier,
      required this.referralFeeBonus,
      required this.bonusStatusType});

  static ReferralStatus from(bridge.ReferralStatus status) {
    return ReferralStatus(
        referralCode: status.referralCode,
        numberOfActivatedReferrals: status.numberOfActivatedReferrals,
        numberOfTotalReferrals: status.numberOfTotalReferrals,
        referralTier: status.referralTier,
        referralFeeBonus: status.referralFeeBonus,
        bonusStatusType: BonusStatusType.from(status.bonusStatusType));
  }
}

enum BonusStatusType {
  none,

  /// The bonus is because he referred enough users
  referral,

  /// The user has been referred and gets a bonus
  referent;

  static BonusStatusType from(bridge.BonusStatusType type) {
    switch (type) {
      case bridge.BonusStatusType.None:
        return BonusStatusType.none;
      case bridge.BonusStatusType.Referral:
        return BonusStatusType.referral;
      case bridge.BonusStatusType.Referent:
        return BonusStatusType.referent;
    }
  }
}
