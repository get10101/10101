import 'package:get_10101/features/stable/stable_screen.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:shared_preferences/shared_preferences.dart';

class Preferences {
  Preferences._privateConstructor();

  static final Preferences instance = Preferences._privateConstructor();

  static const userSeedBackupConfirmed = "userSeedBackupConfirmed";
  static const emailAddress = "emailAddress";
  static const openPosition = "openPosition";

  getOpenPosition() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.getString(openPosition);
  }

  setOpenStablePosition() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.setString(openPosition, StableScreen.label);
  }

  setOpenTradePosition() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.setString(openPosition, TradeScreen.label);
  }

  unsetOpenPosition() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.remove(openPosition);
  }

  setUserSeedBackupConfirmed() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.setBool(userSeedBackupConfirmed, true);
  }

  Future<bool> isUserSeedBackupConfirmed() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.getBool(userSeedBackupConfirmed) ?? false;
  }

  setEmailAddress(String value) async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.setString(emailAddress, value);
  }

  Future<String> getEmailAddress() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.getString(emailAddress) ?? "";
  }

  Future<bool> hasEmailAddress() async {
    var email = await getEmailAddress();
    return email.isNotEmpty;
  }
}
